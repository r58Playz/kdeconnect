#import "headers/CTCall.h"
#import "headers/IOPSKeys.h"
#import "headers/IOPowerSources.h"
#import "headers/MobileGestalt.h"
#import "headers/kern_memorycontrol.h"
#import "headers/CoreTelephonyClient.h"
#import "headers/CTSignalStrengthInfo.h"
#import "headers/CTDataStatus.h"
#import "headers/CTXPCServiceSubscriptionContext.h"
#import "headers/MobileWiFi/MobileWiFi.h"
#import "headers/MPVolumeController.h"
#import "headers/MRContentItem.h"
#import "headers/MRContentItemMetadata.h"
#import "headers/MRArtwork.h"
#import "headers/CTTelephonyCenter.h"
#import "headers/CTCall.h"

#import "kdeconnectjb.h"
#import "server.h"

#import "rootless.h"

#import <Foundation/Foundation.h>
#import <CoreFoundation/CoreFoundation.h>
#import <UIKit/UIKit.h>
#import <IOKit/IOKitLib.h>
#import <AudioToolbox/AudioServices.h>
#import <AVFAudio/AVAudioPlayer.h>
#import <AppSupport/CPDistributedMessagingCenter.h>
#import <unistd.h>
#import <MobileCoreServices/LSApplicationWorkspace.h>
#import <MobileCoreServices/LSApplicationProxy.h>
#import <FrontBoardServices/FBSSystemService.h>
#import <MediaRemote/MediaRemote.h>
#import <CallKit/CXCall.h>
#import <CallKit/CXCallController.h>
#import <CallKit/CXCallObserver.h>

NSString *KDECONNECT_DATA_PATH;
NSString *DOCS_PATH;

NSMutableArray *TRUSTED_NETWORKS;
NSString *CURRENT_CLIPBOARD = @"";

float CURRENT_VOLUME = 0.0f;
double CURRENT_MEDIA_POSITION = 0.0f;

CPDistributedMessagingCenter *appMessageCenter;

CoreTelephonyClient *telephonyClient;

WiFiManagerRef wifiManager;
WiFiDeviceClientRef wifiClient;

MPVolumeController *volumeClient;

NSString *getDeviceId() {
	NSString *uuid = (__bridge NSString *)MGCopyAnswer(
			kMGUniqueDeviceID, (__bridge CFDictionaryRef)(@{}));
	uuid = [uuid stringByReplacingOccurrencesOfString:@"-" withString:@""];
	uuid = [uuid stringByReplacingOccurrencesOfString:@"_" withString:@""];
	return uuid;
}

typedef struct {
	int level;
	int charging;
} BatteryInfo;

NSString *getWifiNetworkSsid() {
	WiFiNetworkRef network = WiFiDeviceClientCopyCurrentNetwork(wifiClient);
	return (__bridge_transfer NSString *)WiFiNetworkGetSSID(network);
}

void trySendMessageToApp(NSString *msg, NSDictionary *info) {
	if (![appMessageCenter sendMessageName:msg userInfo:info]) {
		// reconnect
		appMessageCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.app"];
		[appMessageCenter sendMessageName:msg userInfo:info];
	}
}

void trySendRefreshToApp() {
	trySendMessageToApp(@"refresh", nil);
}

bool getBatteryInfo(BatteryInfo *info) {
	CFTypeRef powerInfo = IOPSCopyPowerSourcesInfo();
	if (!powerInfo) return false;
	CFArrayRef powerSourcesList = IOPSCopyPowerSourcesList(powerInfo);
	if (!powerSourcesList) return false;

	if (CFArrayGetCount(powerSourcesList)) {
		CFDictionaryRef powerSourceInfo = IOPSGetPowerSourceDescription(powerInfo, CFArrayGetValueAtIndex(powerSourcesList, 0));

		CFNumberRef capacityRef = (CFNumberRef)CFDictionaryGetValue(powerSourceInfo, CFSTR("Current Capacity"));
		uint32_t capacity;
		if (!CFNumberGetValue(capacityRef, kCFNumberSInt32Type, &capacity)) return false;

		CFBooleanRef isCharging = (CFBooleanRef) CFDictionaryGetValue(powerSourceInfo, CFSTR("Is Charging"));
		info->level = capacity;
		info->charging = CFBooleanGetValue(isCharging);
		return true;
	}

	return false;
}

NSString *idToContainer(char *bundle)
{
  return [LSApplicationProxy applicationProxyForIdentifier:[NSString stringWithUTF8String:bundle]].dataContainerURL.absoluteString;
}

void powerSourceCallback(void *context) {
  BatteryInfo info = {.level = 0, .charging = 0};
  if (getBatteryInfo(&info)
      && !kdeconnect_on_battery_event(
        info.level,
        info.charging,
        IOPSGetBatteryWarningLevel() != kIOPSLowBatteryWarningNone)
  ) {
    NSLog(@"battery event failed");
  }
}

void mediaCallback() {
  MRMediaRemoteGetNowPlayingInfo(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^(CFDictionaryRef result) {
    if (result) {
      MRContentItem *item = [[MRContentItem alloc] initWithNowPlayingInfo:(__bridge NSDictionary *)result];
      NSLog(@"mpris playing info: %@", item);
      MRArtwork *artwork = item.artwork;
      NSString *path = [DOCS_PATH stringByAppendingString:@"album_art/self.png"];
      if (artwork) {
        UIImage* image = [UIImage imageWithData:artwork.imageData];
        CIImage* imageCI = [CIImage imageWithCGImage:image.CGImage];
        NSData* pngData = [[CIContext context] PNGRepresentationOfImage:imageCI format:kCIFormatRGBA8 colorSpace:CGColorSpaceCreateWithName(kCGColorSpaceSRGB) options:@{}];
        [pngData writeToFile:path atomically:YES];
      } else {
        path = @"";
      }
      NSString *title = item.metadata.title;
      if (!title) title = @"";
      NSString *artist = item.metadata.trackArtistName;
      if (!artist) artist = @"";
      NSString *album = item.metadata.albumName;
      if (!album) album = @"";
      NSLog(@"playing %@", item.nowPlayingInfo);
	  CURRENT_MEDIA_POSITION = item.metadata.calculatedPlaybackPosition;
      kdeconnect_add_player(
        title.UTF8String,
        artist.UTF8String,
        album.UTF8String,
        item.metadata.playbackRate > 0.0001,
        (int)(item.metadata.calculatedPlaybackPosition * 1000),
        (int)(item.metadata.duration * 1000),
        path.UTF8String
      );
    } else {
      kdeconnect_remove_player();
    }
  });
}

void telephonyCallback(CFNotificationCenterRef center, void *observer, CFStringRef name, const void *object, CFDictionaryRef userInfo) {
	NSLog(@"telephony info recieved %@ %@", name, userInfo);

	if (CFStringCompare(name, kCTCallIdentificationChangeNotification, 0) == kCFCompareEqualTo ||
		CFStringCompare(name, kCTCallStatusChangeNotification, 0) == kCFCompareEqualTo) {

		CTCallRef call = (CTCallRef)object;

		CTCallStatus callStatus = (CTCallStatus)[[(__bridge NSDictionary *)userInfo objectForKey:@"kCTCallStatus"] integerValue];
		NSString *callName = (__bridge NSString *)CTCallCopyName(kCFAllocatorDefault, call);

		NSString *callAddress = (__bridge NSString *)CTCallCopyAddress(kCFAllocatorDefault, call);
		NSString *callCountryCode = (__bridge NSString *)CTCallCopyCountryCode(kCFAllocatorDefault, call);
		NSString *callNetworkCode = (__bridge NSString *)CTCallCopyNetworkCode(kCFAllocatorDefault, call);

		NSLog(@"recieved call status for %@: %@ %@ %@", callName, callAddress, callCountryCode, callNetworkCode);

		if (!callAddress) return;
		switch (callStatus) {
			case kCTCallStatusIncomingCall:
				kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_RINGING, callAddress.UTF8String, callAddress.UTF8String, false);
				break;
			case kCTCallStatusIncomingCallEnded:
				kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_RINGING, callAddress.UTF8String, callAddress.UTF8String, true);
				kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_MISSED_CALL, callAddress.UTF8String, callAddress.UTF8String, false);
				break;
			case kCTCallStatusAnswered:
				kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_RINGING, callAddress.UTF8String, callAddress.UTF8String, true);
				kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_TALKING, callAddress.UTF8String, callAddress.UTF8String, false);
				break;
			default:
				break;
		}
	}
}

@interface CXCallController (Private)
- (void)setCallObserver:(CXCallObserver *)arg1;
@end

@interface CallMonitorCallObserverDelegate : NSObject <CXCallObserverDelegate>
@end

@implementation CallMonitorCallObserverDelegate

- (void)callObserver:(CXCallObserver *)callObserver callChanged:(CXCall *)call {
	if (!call.outgoing && !call.onHold && !call.hasConnected && !call.hasEnded) {
		kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_RINGING, "iOS CallKit App", call.UUID.UUIDString.UTF8String, false);
	} else if (!call.outgoing && !call.onHold && call.hasConnected && !call.hasEnded) {
		kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_RINGING, "iOS CallKit App", call.UUID.UUIDString.UTF8String, true);
		kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_TALKING, "iOS CallKit App", call.UUID.UUIDString.UTF8String, false);
	} else if (!call.outgoing && !call.onHold && !call.hasConnected && call.hasEnded) {
		kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_RINGING, "iOS CallKit App", call.UUID.UUIDString.UTF8String, true);
		kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_MISSED_CALL, "iOS CallKit App", call.UUID.UUIDString.UTF8String, false);
	} else if (!call.outgoing && !call.onHold && call.hasConnected && call.hasEnded) {
		kdeconnect_send_telephony_update(K_CONNECT_TELEPHONY_EVENT_TALKING, "iOS CallKit App", call.UUID.UUIDString.UTF8String, true);
	}
}

@end

void initialized_callback() { 
  NSLog(@"initialized, sending data & telling app");
  powerSourceCallback(NULL);

  NSString *clipboard = NULL;
  if ((clipboard = UIPasteboard.generalPasteboard.string)) {
    kdeconnect_on_clipboard_event((char*)clipboard.UTF8String);
  }
  CURRENT_CLIPBOARD = clipboard;

  trySendRefreshToApp();
}

void discovered_callback(char* device_id) {
  NSLog(@"discovered");
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"retrieved discovered device: %s", device_by_id->name);
    kdeconnect_device_request_volume(device_by_id);
    trySendRefreshToApp();
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void gone_callback(char *device_id) {
  trySendRefreshToApp();
  kdeconnect_free_string(device_id);
}

bool pairing_callback(char* device_id, char* device_key) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
	if (device_by_id) {
    NSLog(@"retrieved device that wants to pair: %s", device_by_id->name);

    NSString *devName = [NSString stringWithUTF8String:device_by_id->name];
    NSString *key = [NSString stringWithUTF8String:device_key];
    NSMutableDictionary *alert = [[NSMutableDictionary alloc] init];

    NSString *message = [[[@"Recieved pairing request from device: " stringByAppendingString:devName] stringByAppendingString:@"\nkey: "] stringByAppendingString: key];

    [alert addEntriesFromDictionary:@{
      (__bridge NSString*)kCFUserNotificationAlertHeaderKey:@"KDE Connect",
      (__bridge NSString*)kCFUserNotificationAlertMessageKey:message,
      (__bridge NSString*)kCFUserNotificationDefaultButtonTitleKey:@"Decline",
      (__bridge NSString*)kCFUserNotificationAlternateButtonTitleKey:@"Accept",
    }];
    CFUserNotificationRef notif = CFUserNotificationCreate(kCFAllocatorDefault, 0, 0, NULL, (__bridge CFMutableDictionaryRef) alert);

    CFOptionFlags cfRes;

    CFUserNotificationReceiveResponse(notif, 30, &cfRes);

    kdeconnect_free_device(device_by_id);
    kdeconnect_free_string(device_id);
    kdeconnect_free_string(device_key);
    return cfRes == 1;
  }
  kdeconnect_free_string(device_id);
  kdeconnect_free_string(device_key);
  return false;
}

void pair_status_changed_callback(char* device_id, bool pair_status) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"device `%s` pair status changed to: %s", device_by_id->name, pair_status ? "paired" : "not paired");
    trySendRefreshToApp();
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void battery_callback(char *device_id) {
  trySendRefreshToApp();
  kdeconnect_free_string(device_id);
}

void clipboard_callback(char *device_id, char *clipboard) {
  NSString *clipboardNS = [NSString stringWithUTF8String:clipboard];
  UIPasteboard.generalPasteboard.string = clipboardNS;
  CURRENT_CLIPBOARD = clipboardNS;

  trySendRefreshToApp();
  kdeconnect_free_string(device_id);
}

void ping_callback(char *device_id) {
  KConnectFfiDevice_t *device_by_id;
  if ((device_by_id = kdeconnect_get_device_by_id(device_id))) {
    NSString *devName = [NSString stringWithUTF8String:device_by_id->name];
    NSMutableDictionary *alert = [[NSMutableDictionary alloc] init];
    [alert addEntriesFromDictionary:@{
      (__bridge NSString*)kCFUserNotificationAlertHeaderKey:@"KDE Connect",
      (__bridge NSString*)kCFUserNotificationAlertMessageKey:[@"Recieved ping from device: " stringByAppendingString:devName],
      (__bridge NSString*)kCFUserNotificationDefaultButtonTitleKey:@"OK",
    }];
    CFUserNotificationRef notif = CFUserNotificationCreate(kCFAllocatorDefault, 0, 0, NULL, (__bridge CFMutableDictionaryRef) alert);

    CFOptionFlags cfRes;

    CFUserNotificationReceiveResponse(notif, 0, &cfRes);
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void find_callback() {
  NSLog(@"i am lost!");

  NSThread *message_thread = [[NSThread alloc] initWithBlock:^void() {
    NSMutableDictionary *alert = [[NSMutableDictionary alloc] init];
    [alert addEntriesFromDictionary:@{
      (__bridge NSString*)kCFUserNotificationAlertHeaderKey:@"KDE Connect",
      (__bridge NSString*)kCFUserNotificationAlertMessageKey:@"Find my device alert",
      (__bridge NSString*)kCFUserNotificationDefaultButtonTitleKey:@"OK",
    }];
    CFUserNotificationRef notif = CFUserNotificationCreate(kCFAllocatorDefault, 0, 0, NULL, (__bridge CFMutableDictionaryRef) alert);

    CFOptionFlags cfRes;

    CFUserNotificationReceiveResponse(notif, 0, &cfRes);
    kdeconnect_set_is_lost(false);
  }];

  [message_thread start];

  [[AVAudioSession sharedInstance] setCategory:AVAudioSessionCategoryPlayback
                                         error:nil];
  [[AVAudioSession sharedInstance] overrideOutputAudioPort:AVAudioSessionPortOverrideSpeaker
                                                     error:nil];

  NSURL *url = [NSURL fileURLWithPath:@"/System/Library/PrivateFrameworks/FindMyDevice.framework/fmd_sound.caf"];
  AVAudioPlayer *player = [[AVAudioPlayer alloc] initWithContentsOfURL:url
                                                                 error:nil];
  player.numberOfLoops = -1;

  [player play];
  while (kdeconnect_get_is_lost()) {
    AudioServicesPlaySystemSound(kSystemSoundID_Vibrate);
    usleep(500000);
  }
  [player stop];

  NSLog(@"i am no longer lost!");
}

void connectivity_callback(char *device_id) {
  trySendRefreshToApp();
  kdeconnect_free_string(device_id);
}

void device_volume_callback(char *device_id) {
  trySendRefreshToApp();
  kdeconnect_free_string(device_id);
}

void volume_change_callback(int vol) {
  float newVol = ((float)vol) / 100.0f;
  [volumeClient setVolume:newVol withNotificationDelay:0.0f];
}

void open_file_callback(char *path) {
  NSLog(@"file saved path %s", path);
  NSURL *url = [NSURL URLWithString:[@"shareddocuments://" stringByAppendingString:[NSString stringWithUTF8String:path]]];
  [[LSApplicationWorkspace defaultWorkspace] openSensitiveURL:url withOptions:@{ FBSOpenApplicationOptionKeyUnlockDevice: @YES }];
}

void open_url_callback(char *url) {
  NSURL *urlNS = [[[NSURLComponents alloc] initWithString:[NSString stringWithUTF8String:url]] URL];
  NSLog(@"opening url %s %@", url, urlNS);
  [[LSApplicationWorkspace defaultWorkspace] openSensitiveURL:urlNS withOptions:@{ FBSOpenApplicationOptionKeyUnlockDevice: @YES }];
}

void open_text_callback(char *text) {
  NSString *textNS = [NSString stringWithUTF8String:text];
  UIPasteboard.generalPasteboard.string = textNS;
  CURRENT_CLIPBOARD = textNS;
}

void player_callback(char *device_id) {
  trySendRefreshToApp();
  kdeconnect_free_string(device_id);
}

void player_action_callback(KConnectMprisPlayerAction_t action, int64_t val) {
	NSLog(@"got action %d %lld", action, val);
	switch (action) {
		case K_CONNECT_MPRIS_PLAYER_ACTION_SEEK:
			MRMediaRemoteSetElapsedTime(CURRENT_MEDIA_POSITION + (((double)val) / 1000.0f));
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_VOLUME:
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_LOOP_STATUS_NONE:
			MRMediaRemoteSetRepeatMode(0);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_LOOP_STATUS_TRACK:
			MRMediaRemoteSetRepeatMode(1);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_LOOP_STATUS_PLAYLIST:
			MRMediaRemoteSetRepeatMode(2);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_POSITION:
			MRMediaRemoteSetElapsedTime(((double)val) / 1000.0f);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_PLAY:
			MRMediaRemoteSendCommand(MRMediaRemoteCommandPlay, nil);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_PAUSE:
			MRMediaRemoteSendCommand(MRMediaRemoteCommandPause, nil);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_PLAY_PAUSE:
			MRMediaRemoteSendCommand(MRMediaRemoteCommandTogglePlayPause, nil);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_STOP:
			MRMediaRemoteSendCommand(MRMediaRemoteCommandStop, nil);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_NEXT:
			MRMediaRemoteSendCommand(MRMediaRemoteCommandNextTrack, nil);
			break;
		case K_CONNECT_MPRIS_PLAYER_ACTION_PREVIOUS:
			MRMediaRemoteSendCommand(MRMediaRemoteCommandPreviousTrack, nil);
			break;
	}
}

int objc_main(const char *deviceName, KConnectFfiDeviceType_t deviceType) {
  @autoreleasepool {
    KDECONNECT_DATA_PATH = @"/var/mobile/kdeconnect";
    NSString *trustedPath = [KDECONNECT_DATA_PATH stringByAppendingString:@"/trusted"];
    if (![[NSFileManager defaultManager] fileExistsAtPath:trustedPath]) {
      [[NSFileManager defaultManager] createFileAtPath:trustedPath contents:[NSData data] attributes:nil];
    }
    TRUSTED_NETWORKS = [[[NSString stringWithContentsOfFile:trustedPath encoding:NSUTF8StringEncoding error:nil] componentsSeparatedByCharactersInSet:[NSCharacterSet newlineCharacterSet]] mutableCopy];
    [TRUSTED_NETWORKS removeObject:@""];
    NSLog(@"trustedNetworks: %@", TRUSTED_NETWORKS);
    DOCS_PATH = [[idToContainer("dev.r58playz.kdeconnectjb") stringByReplacingOccurrencesOfString:@"file://" withString:@""] stringByAppendingString:@"Documents/"];
    NSLog(@"saved files path: %@", DOCS_PATH);

    wifiManager = WiFiManagerClientCreate(kCFAllocatorDefault, 0);
    CFArrayRef devices = WiFiManagerClientCopyDevices(wifiManager);
    if (!devices) {
      return 0;
    }
    wifiClient = (WiFiDeviceClientRef)CFArrayGetValueAtIndex(devices, 0);

    if (TRUSTED_NETWORKS.count) {
      NSString *ssid = getWifiNetworkSsid();
      while (!ssid || ![TRUSTED_NETWORKS containsObject:ssid]) {
        NSLog(@"Waiting 60 seconds for trusted network");
        usleep(60000000);
        ssid = getWifiNetworkSsid();
      }
    }

    NSLog(@"[Flotsam:INFO] Hammer time.");
    pid_t pid = getpid();
    // make it jetsamable if it's really in low memory
    memorystatus_priority_properties_t props = {JETSAM_PRIORITY_AUDIO_AND_ACCESSORY, 0};

    if (memorystatus_control(MEMORYSTATUS_CMD_SET_PRIORITY_PROPERTIES, pid, 0,
                             &props, sizeof(props)) != 0) {
      NSLog(@"[Flotsam:WARN] Could not set jetsam priority for process %d. "
            @"(%d)",
            pid, errno);
    } else {
      NSLog(@"[Flotsam:INFO] Set jetsam priority for process %d to %d.", pid,
            props.priority);
    }

    if (memorystatus_control(MEMORYSTATUS_CMD_SET_JETSAM_HIGH_WATER_MARK, pid,
                             -1, NULL, 0) != 0) {
      NSLog(@"[Flotsam:WARN] Could not set jetsam high water mark on process "
            @"%d. (%d)",
            pid, errno);
    } else {
      NSLog(@"[Flotsam:INFO] Set jetsam high water mark on process %d to -1.",
            pid);
    }

    if (memorystatus_control(MEMORYSTATUS_CMD_SET_JETSAM_TASK_LIMIT, pid,
                             -1, NULL, 0) != 0) {
      NSLog(@"[Flotsam:WARN] Could not set jetsam task limit on process "
            @"%d. (%d)",
            pid, errno);
    } else {
      NSLog(@"[Flotsam:INFO] Set jetsam task limit on process %d to -1.",
            pid);
    }

    if (memorystatus_control(MEMORYSTATUS_CMD_SET_PROCESS_IS_MANAGED, pid, 0,
                             NULL, 0) != 0) {
      NSLog(@"[Flotsam:WARN] Could not set process %d as unmanaged. (%d)",
            pid, errno);
    } else {
      NSLog(@"[Flotsam:INFO] Set process %d as unmanaged.", pid);
    }

    if (memorystatus_control(MEMORYSTATUS_CMD_SET_PROCESS_IS_FREEZABLE, pid,
                             0, NULL, 0) != 0) {
      NSLog(@"[Flotsam:WARN] Could not set process %d as non-freezable. (%d)",
            pid, errno);
    } else {
      NSLog(@"[Flotsam:INFO] Set process %d as non-freezable.", pid);
    }

    appMessageCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.app"];

    CoreTelephonyClientMux *telephonyClientMux = [CoreTelephonyClient sharedMultiplexer];
    telephonyClient = [[CoreTelephonyClient alloc] init];
    [telephonyClient setMux:telephonyClientMux];

    volumeClient = [[MPVolumeController alloc] init];

    NSString *deviceId = getDeviceId();
    if (!deviceId) {
      NSLog(@"error: No device id\n");
      return 1;
    }

    if (!kdeconnect_init()) {
      NSLog(@"error: failed to init kdeconnect");
      return 1;
    }

    kdeconnect_register_init_callback(initialized_callback);
    kdeconnect_register_discovered_callback(discovered_callback);
    kdeconnect_register_gone_callback(gone_callback);
    kdeconnect_register_pairing_callback(pairing_callback);
    kdeconnect_register_pair_status_changed_callback(pair_status_changed_callback);
    kdeconnect_register_battery_callback(battery_callback);
    kdeconnect_register_clipboard_callback(clipboard_callback);
    kdeconnect_register_ping_callback(ping_callback);
    kdeconnect_register_find_callback(find_callback);
    kdeconnect_register_connectivity_callback(connectivity_callback);
    kdeconnect_register_volume_change_callback(volume_change_callback);
    kdeconnect_register_device_volume_callback(device_volume_callback);
    kdeconnect_register_open_file_callback(open_file_callback);
    kdeconnect_register_open_url_callback(open_url_callback);
    kdeconnect_register_open_text_callback(open_text_callback);
    kdeconnect_register_player_change_callback(player_callback);
	kdeconnect_register_player_action_callback(player_action_callback);

    NSThread *kdeconnect_thread = [[NSThread alloc] initWithBlock:^void() {
      bool res = kdeconnect_start(
        (char*)deviceId.UTF8String,
        deviceName,
        deviceType,
        (char*)KDECONNECT_DATA_PATH.UTF8String,
        (char*)DOCS_PATH.UTF8String
      );
      NSLog(@"Ended OK: %d\n", res);
      exit(res);
    }];

    [kdeconnect_thread start];

    NSThread *message_thread = [[NSThread alloc] initWithBlock:^void() {
      [KConnectServer load];
    }];

    [message_thread start];

    // TODO: Replace with Pasteboard.framework?
    CFRunLoopTimerRef clipboardLoop = CFRunLoopTimerCreateWithHandler(NULL, CFAbsoluteTimeGetCurrent(), 1.0, 0, 0, ^(CFRunLoopTimerRef timer){
        NSString *clipboard = UIPasteboard.generalPasteboard.string;
        if (!clipboard) clipboard = @"";

        if (![clipboard isEqualToString:CURRENT_CLIPBOARD]) {
          NSLog(@"clipboard changed");
          kdeconnect_on_clipboard_event(clipboard.UTF8String);
          CURRENT_CLIPBOARD = clipboard;
        }
    });

    CFRunLoopTimerRef connectivityLoop = CFRunLoopTimerCreateWithHandler(NULL, CFAbsoluteTimeGetCurrent(), 60.0, 0, 0, ^(CFRunLoopTimerRef timer){
      // TODO: Multiple sims
      CTXPCServiceSubscriptionContext *context = [CTXPCServiceSubscriptionContext contextWithSlot:1];
      CTSignalStrengthInfo *info = [telephonyClient getSignalStrengthInfo:context error:NULL];
      NSString *radioTechnology = [telephonyClient copyRadioAccessTechnology:context error:NULL];
      kdeconnect_clear_connectivity_signals();
      if (info && radioTechnology) {
        kdeconnect_add_connectivity_signal("1", radioTechnology.UTF8String, info.bars.intValue);
        NSLog(@"added connectivity signal: 1 %@ %d", radioTechnology, info.bars.intValue);
      }
      kdeconnect_send_connectivity_update();
    });

    CFRunLoopTimerRef trustedNetworkLoop = CFRunLoopTimerCreateWithHandler(NULL, CFAbsoluteTimeGetCurrent(), 10.0, 0, 0, ^(CFRunLoopTimerRef timer){
      NSString *ssid = getWifiNetworkSsid();
      if (TRUSTED_NETWORKS.count && (!ssid || ![TRUSTED_NETWORKS containsObject:ssid])) {
        NSLog(@"no longer on trusted network!!");
        exit(0);
      }
    });
    
    CFRunLoopTimerRef volumeLoop = CFRunLoopTimerCreateWithHandler(NULL, CFAbsoluteTimeGetCurrent(), 10.0, 0, 0, ^(CFRunLoopTimerRef timer){
      [volumeClient getVolumeValueWithCompletion:^(float resp){
        if (resp != CURRENT_VOLUME) {
          kdeconnect_set_volume((int)(resp * 100));
          CURRENT_VOLUME = resp;
        }
      }];
    });

    [[NSNotificationCenter defaultCenter] addObserverForName:@"kMRMediaRemoteNowPlayingInfoDidChangeNotification" object:nil queue:nil usingBlock:^(NSNotification *notif) {
      mediaCallback();
    }];

    MRMediaRemoteRegisterForNowPlayingNotifications(dispatch_get_main_queue());

	CTTelephonyCenterAddObserver(CTTelephonyCenterGetDefault(), NULL, telephonyCallback,
		kCTCallStatusChangeNotification, NULL,
		CFNotificationSuspensionBehaviorDeliverImmediately);

	CTTelephonyCenterAddObserver(CTTelephonyCenterGetDefault(), NULL, telephonyCallback,
		kCTCallIdentificationChangeNotification, NULL,
		CFNotificationSuspensionBehaviorDeliverImmediately);

	CXCallController *callController = [[CXCallController alloc] initWithQueue:dispatch_get_main_queue()];
	CallMonitorCallObserverDelegate *callObserverDelegate = [CallMonitorCallObserverDelegate new];
	[callController.callObserver setDelegate:callObserverDelegate queue:dispatch_get_main_queue()];

    CFRunLoopSourceRef powerLoop =
        IOPSNotificationCreateRunLoopSource(powerSourceCallback, NULL);
    CFRunLoopAddSource(CFRunLoopGetMain(), powerLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), clipboardLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), connectivityLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), trustedNetworkLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), volumeLoop, kCFRunLoopDefaultMode);

    CFRunLoopRun();

    return 0;
  }
}

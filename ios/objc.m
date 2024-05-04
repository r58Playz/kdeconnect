// vim: tabstop=2 shiftwidth=2
#import "headers/IOPSKeys.h"
#import "headers/IOPowerSources.h"
#import "headers/MobileGestalt.h"
#import "headers/kern_memorycontrol.h"
#import "headers/CoreTelephonyClient.h"
#import "headers/CTSignalStrengthInfo.h"
#import "headers/CTDataStatus.h"
#import "headers/CTXPCServiceSubscriptionContext.h"
#import "headers/MobileWiFi/MobileWiFi.h"

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

bool TROLLSTORE = false;
NSString *KDECONNECT_DATA_PATH;

NSMutableArray *TRUSTED_NETWORKS;
NSString *CURRENT_CLIPBOARD = @"";

CPDistributedMessagingCenter *tweakMessageCenter;
CPDistributedMessagingCenter *appMessageCenter;

CoreTelephonyClient *telephonyClient;

WiFiManagerRef wifiManager;
WiFiDeviceClientRef wifiClient;

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

void initialized_callback() { 
  NSLog(@"initialized, sending data & telling app");
  powerSourceCallback(NULL);

  NSString *clipboard = NULL;
  if ((clipboard = UIPasteboard.generalPasteboard.string)) {
    kdeconnect_on_clipboard_event((char*)clipboard.UTF8String);
  }

  [appMessageCenter sendMessageName:@"refresh" userInfo:nil];
}

void discovered_callback(char* device_id) {
  NSLog(@"discovered");
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"retrieved discovered device: %s", device_by_id->name);
    [appMessageCenter sendMessageName:@"refresh" userInfo:nil];
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

bool pairing_callback(char* device_id) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"retrieved device that wants to pair: %s", device_by_id->name);

    NSString *devName = [NSString stringWithUTF8String:device_by_id->name];
    NSMutableDictionary *alert = [[NSMutableDictionary alloc] init];
    [alert addEntriesFromDictionary:@{
      (__bridge NSString*)kCFUserNotificationAlertHeaderKey:@"KDE Connect",
      (__bridge NSString*)kCFUserNotificationAlertMessageKey:[@"Recieved pairing request from device: " stringByAppendingString:devName],
      (__bridge NSString*)kCFUserNotificationDefaultButtonTitleKey:@"Decline",
      (__bridge NSString*)kCFUserNotificationAlternateButtonTitleKey:@"Accept",
    }];
    CFUserNotificationRef notif = CFUserNotificationCreate(kCFAllocatorDefault, 0, 0, NULL, (__bridge CFMutableDictionaryRef) alert);

    CFOptionFlags cfRes;

    CFUserNotificationReceiveResponse(notif, 30, &cfRes);

    kdeconnect_free_device(device_by_id);
    kdeconnect_free_string(device_id);
    return cfRes == 1;
  }
  kdeconnect_free_string(device_id);
  return false;
}

void pair_status_changed_callback(char* device_id, bool pair_status) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"device `%s` pair status changed to: %s", device_by_id->name, pair_status ? "paired" : "not paired");
    [appMessageCenter sendMessageName:@"refresh" userInfo:nil];
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void battery_callback(char *device_id) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"device sent battery data: %s", device_by_id->name);
    [appMessageCenter sendMessageName:@"refresh" userInfo:nil];
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void clipboard_callback(char *device_id, char *clipboard) {
  UIPasteboard.generalPasteboard.string = [NSString stringWithUTF8String:clipboard];

  [appMessageCenter sendMessageName:@"refresh" userInfo:nil];
  kdeconnect_free_string(device_id);
}

void ping_callback(char *device_id) {
  KConnectFfiDevice_t *device_by_id;
  if (!TROLLSTORE && 
      (device_by_id = kdeconnect_get_device_by_id(device_id))) {
    [tweakMessageCenter sendMessageName:@"ping" userInfo:@{ @"name":[NSString stringWithUTF8String:device_by_id->name] }];
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void find_callback() {
  NSLog(@"i am lost!");
  if (!TROLLSTORE) {
    [tweakMessageCenter sendMessageName:@"lost" userInfo:nil];
    NSLog(@"sent message to tweak telling it i am lost!");
  }

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

int objc_main(const char *deviceName, KConnectFfiDeviceType_t deviceType, bool trollstore) {
  @autoreleasepool {
    KDECONNECT_DATA_PATH = ROOT_PATH_NS(@"/var/mobile/kdeconnect");
    NSString *trustedPath = [KDECONNECT_DATA_PATH stringByAppendingString:@"/trusted"];
    if (![[NSFileManager defaultManager] fileExistsAtPath:trustedPath]) {
      [[NSFileManager defaultManager] createFileAtPath:trustedPath contents:[NSData data] attributes:nil];
    }
    TRUSTED_NETWORKS = [[[NSString stringWithContentsOfFile:trustedPath encoding:NSUTF8StringEncoding error:nil] componentsSeparatedByCharactersInSet:[NSCharacterSet newlineCharacterSet]] mutableCopy];
    [TRUSTED_NETWORKS removeLastObject];
    NSLog(@"trustedNetworks: %@", TRUSTED_NETWORKS);

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

    TROLLSTORE = trollstore;
    if (trollstore) {
      NSLog(@"Starting as TrollStore daemon");
      NSLog(@"[Flotsam:INFO] Hammer time.");
      pid_t pid = getpid();
      memorystatus_priority_properties_t props = {JETSAM_PRIORITY_CRITICAL, 0};

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
    } else {
      NSLog(@"Starting as JB daemon");
      tweakMessageCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.springboard"];
    }
    appMessageCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.app"];

    CoreTelephonyClientMux *telephonyClientMux = [CoreTelephonyClient sharedMultiplexer];
    telephonyClient = [[CoreTelephonyClient alloc] init];
    [telephonyClient setMux:telephonyClientMux];

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
    kdeconnect_register_pairing_callback(pairing_callback);
    kdeconnect_register_pair_status_changed_callback(pair_status_changed_callback);
    kdeconnect_register_battery_callback(battery_callback);
    kdeconnect_register_clipboard_callback(clipboard_callback);
    kdeconnect_register_ping_callback(ping_callback);
    kdeconnect_register_find_callback(find_callback);

    NSThread *kdeconnect_thread = [[NSThread alloc] initWithBlock:^void() {
      bool res = kdeconnect_start(
        (char*)deviceId.UTF8String,
        deviceName,
        deviceType,
        (char*)KDECONNECT_DATA_PATH.UTF8String
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
        kdeconnect_add_connectivity_signal("1", radioTechnology.UTF8String, info.bars.intValue + 1);
        NSLog(@"added connectivity signal: 1 %@ %d", radioTechnology, info.bars.intValue + 1);
      }
    });

    CFRunLoopTimerRef trustedNetworkLoop = CFRunLoopTimerCreateWithHandler(NULL, CFAbsoluteTimeGetCurrent(), 10.0, 0, 0, ^(CFRunLoopTimerRef timer){
      NSString *ssid = getWifiNetworkSsid();
      if (TRUSTED_NETWORKS.count && (!ssid || ![TRUSTED_NETWORKS containsObject:ssid])) {
        NSLog(@"no longer on trusted network!!");
        abort();
      }
    });

    CFRunLoopSourceRef powerLoop =
        IOPSNotificationCreateRunLoopSource(powerSourceCallback, NULL);
    CFRunLoopAddSource(CFRunLoopGetMain(), powerLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), clipboardLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), connectivityLoop, kCFRunLoopDefaultMode);
    CFRunLoopAddTimer(CFRunLoopGetMain(), trustedNetworkLoop, kCFRunLoopDefaultMode);

    CFRunLoopRun();

    return 0;
  }
}

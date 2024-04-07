// vim: tabstop=2 shiftwidth=2
#import "headers/IOPSKeys.h"
#import "headers/IOPowerSources.h"
#import "headers/MobileGestalt.h"
#import "headers/kern_memorycontrol.h"

#import "kdeconnectjb.h"
#import "server.h"

#import "rootless.h"

#import <Foundation/Foundation.h>
#import <CoreFoundation/CoreFoundation.h>
#import <UIKit/UIKit.h>
#import <IOKit/IOKitLib.h>
#import <AudioToolbox/AudioServices.h>
#import <AVFAudio/AVAudioPlayer.h>
#import <CoreTelephony/CTTelephonyNetworkInfo.h>
#import <AppSupport/CPDistributedMessagingCenter.h>
#import <unistd.h>

bool TROLLSTORE = false;
NSString *KDECONNECT_DATA_PATH;
CPDistributedMessagingCenter *tweakMessageCenter;

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

NSDictionary *getPMDict() {
  CFDictionaryRef matching = IOServiceMatching("IOPMPowerSource");
  io_service_t service =
      IOServiceGetMatchingService(kIOMasterPortDefault, matching);
  CFMutableDictionaryRef prop = NULL;
  IORegistryEntryCreateCFProperties(service, &prop, NULL, 0);
  NSDictionary *dict = (__bridge_transfer NSDictionary *)prop;
  IOObjectRelease(service);
  return dict;
}

bool getBatteryInfo(BatteryInfo *info) {
  NSDictionary *dict = getPMDict();
  if (!dict) {
    return false;
  }

  info->charging = 0;
  info->level = 0.0f;

  info->charging = [dict[@"ExternalChargeCapable"] intValue];
  info->level = [dict[@"CurrentCapacity"] intValue];

  return true;
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
  NSLog(@"initialized, sending battery data");
  powerSourceCallback(NULL);
}

void discovered_callback(char* device_id) {
  NSLog(@"discovered");
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"retrieved discovered device: %s", device_by_id->name);
    // TODO: Send this over to the app (@bomberfish)
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

bool pairing_callback(char* device_id) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"retrieved device that wants to pair: %s", device_by_id->name);
    // TODO: Ask user via alert here (@bomberfish)
    kdeconnect_free_device(device_by_id);
    kdeconnect_free_string(device_id);
    NSLog(@"blindly accepting pair");
    return true;
  }
  kdeconnect_free_string(device_id);
  return false;
}

void pair_status_changed_callback(char* device_id, bool pair_status) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"device `%s` pair status changed to: %s", device_by_id->name, pair_status ? "paired" : "not paired");
    // TODO: Send this over to the app (@bomberfish)
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void battery_callback(char *device_id) {
  KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device_id);
  if (device_by_id) {
    NSLog(@"device sent battery data: %s", device_by_id->name);
    // TODO: Send this over to the app (@bomberfish)
    kdeconnect_free_device(device_by_id);
  }
  kdeconnect_free_string(device_id);
}

void clipboard_callback(char *device_id, char *clipboard) {
  NSLog(@"clipboard data recieved from device_id: `%s` data: `%s`", device_id, clipboard);
  // TODO: Set system clipboard here
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

    KDECONNECT_DATA_PATH = ROOT_PATH_NS(@"/var/mobile/kdeconnect");
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
        (char*)[deviceId cStringUsingEncoding:NSUTF8StringEncoding],
        deviceName,
        deviceType,
        (char*)[KDECONNECT_DATA_PATH cStringUsingEncoding:NSUTF8StringEncoding]
      );
      NSLog(@"Ended OK: %d\n", res);
      exit(res);
    }];

    [kdeconnect_thread start];

    NSThread *message_thread = [[NSThread alloc] initWithBlock:^void() {
      [KConnectServer load];
    }];

    [message_thread start];

    // TODO: Subscribe to clipboard events and send those over
    CFRunLoopSourceRef powerLoop =
        IOPSNotificationCreateRunLoopSource(powerSourceCallback, NULL);
    CFRunLoopAddSource(CFRunLoopGetMain(), powerLoop, kCFRunLoopDefaultMode);

    CFRunLoopRun();

    return 0;
  }
}

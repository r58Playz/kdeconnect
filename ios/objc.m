#import "headers/IOPSKeys.h"
#import "headers/IOPowerSources.h"
#import "headers/MobileGestalt.h"
#import "headers/kern_memorycontrol.h"

#import "kdeconnectjb.h"

#import "rootless.h"

#import <Foundation/Foundation.h>
#import <CoreFoundation/CoreFoundation.h>
#import <UIKit/UIKit.h>
#import <IOKit/IOKitLib.h>
#import <unistd.h>

NSString *KDECONNECT_DATA_PATH;

NSString *getDeviceId() {
  NSString *uuid = (__bridge NSString *)MGCopyAnswer(
      kMGUniqueDeviceID, (__bridge CFDictionaryRef)(@{}));
  uuid = [uuid stringByReplacingOccurrencesOfString:@"-" withString:@""];
  uuid = [uuid stringByReplacingOccurrencesOfString:@"_" withString:@""];
  return uuid;
}

typedef struct {
  double level;
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
  info->level = [dict[@"CurrentCapacity"] intValue] / 100.0f;

  return true;
}

void powerSourceCallback(void *context) {
    BatteryInfo info = {.level = 0.0f, .charging = 0};
    if (getBatteryInfo(&info)
        && !kdeconnect_on_battery_event(
            info.level,
            info.charging,
            IOPSGetBatteryWarningLevel() != kIOPSLowBatteryWarningNone)
    ) {
        NSLog(@"battery event failed");
    }
}

void initialized_callback() { powerSourceCallback(NULL); }

void discovered_callback() {
    NSLog(@"discovered");
    Vec_KConnectFfiDevice_t vec = kdeconnect_get_device_list();
    NSLog(@"got list");
    for (int i = 0; i < vec.len; i++) {
        KConnectFfiDevice_t device = vec.ptr[i];
        NSLog(@"device name: %s id: %s batterylevel: %d", device.name, device.id, kdeconnect_device_get_battery_level(&device));
        KConnectFfiDevice_t *device_by_id = kdeconnect_get_device_by_id(device.id);
        if (device_by_id) {
            NSLog(@"retrieved device by id: %s", device_by_id->name);
            kdeconnect_free_device(device_by_id);
        }
    }
    kdeconnect_free_device_list(vec);
}

int objc_main(int argc, char *argv[]) {
  @autoreleasepool {
    if (argc != 2 && argc != 3) {
      NSLog(@"usage: %s <device_name> [--trollstore]\n", argv[0]);
      return 1;
    }

    if (argc == 3 && strcmp(argv[2], "--trollstore")) {
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

    NSThread *thread = [[NSThread alloc] initWithBlock:^void() {
      bool res = kdeconnect_start(
          (char*)[deviceId cStringUsingEncoding:NSUTF8StringEncoding],
          argv[1],
          K_CONNECT_FFI_DEVICE_TYPE_PHONE,
          (char*)[KDECONNECT_DATA_PATH cStringUsingEncoding:NSUTF8StringEncoding],
          initialized_callback,
          discovered_callback
      );
      NSLog(@"Ended OK: %d\n", res);
      exit(res);
    }];

    [thread start];

    CFRunLoopSourceRef powerLoop =
        IOPSNotificationCreateRunLoopSource(powerSourceCallback, NULL);
    CFRunLoopAddSource(CFRunLoopGetMain(), powerLoop, kCFRunLoopDefaultMode);

    CFRunLoopRun();

    return 0;
  }
}

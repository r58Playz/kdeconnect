#import "headers/MobileGestalt.h"
#import "headers/kern_memorycontrol.h"
#import "headers/IOPowerSources.h"
#import "headers/IOPSKeys.h"

#import "kdeconnectjb.h"

#import "rootless.h"

#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <unistd.h>

NSString *KDECONNECT_DATA_PATH;

NSString *getDeviceId() {
    NSString *uuid = (__bridge NSString *)MGCopyAnswer(kMGUniqueDeviceID, (__bridge CFDictionaryRef)(@{}));
    uuid = [uuid stringByReplacingOccurrencesOfString:@"-" withString:@""];
    uuid = [uuid stringByReplacingOccurrencesOfString:@"_" withString:@""];
    return uuid;
}

typedef struct {
    double level;
    int charging;
} BatteryInfo;

bool getBatteryInfo(BatteryInfo *info) {
    CFTypeRef blob = IOPSCopyPowerSourcesInfo();
    CFArrayRef sources = IOPSCopyPowerSourcesList(blob);
    
    CFDictionaryRef pSource = NULL;
    const void *psValue;

    int numOfSources = CFArrayGetCount(sources);
    if (numOfSources == 0) {
        NSLog(@"Error in CFArrayGetCount");
        return NO;
    }

    for (int i = 0; i < numOfSources; i++) {
        pSource = IOPSGetPowerSourceDescription(blob, CFArrayGetValueAtIndex(sources, i));
        if (!pSource) {
            NSLog(@"Error in IOPSGetPowerSourceDescription");
            return NO;
        }
        psValue = (CFStringRef)CFDictionaryGetValue(pSource, CFSTR(kIOPSNameKey));

        int curCapacity = 0;
        int maxCapacity = 0;
        double percent;

        psValue = CFDictionaryGetValue(pSource, CFSTR(kIOPSCurrentCapacityKey));
        CFNumberGetValue((CFNumberRef)psValue, kCFNumberSInt32Type, &curCapacity);

        psValue = CFDictionaryGetValue(pSource, CFSTR(kIOPSMaxCapacityKey));
        CFNumberGetValue((CFNumberRef)psValue, kCFNumberSInt32Type, &maxCapacity);

        psValue = CFDictionaryGetValue(pSource, CFSTR(kIOPSIsChargingKey));
        bool isCharging = CFBooleanGetValue((CFBooleanRef)psValue);

        percent = ((double)curCapacity / (double)maxCapacity * 100.0f);

        info->level = percent;
        info->charging = isCharging;

        return YES;
    }
    return NO;
}

void powerSourceCallback(void *context) {
    BatteryInfo info = { .level = 0.0f, .charging = 0 };
    if (getBatteryInfo(&info) && !kdeconnect_on_battery_event(info.level, info.charging, 0)) {
        NSLog(@"battery event failed");
    }
}

void initialized_callback() {
    powerSourceCallback(NULL);
}

int main(int argc, char *argv[], char *envp[]) {
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
          [deviceId cStringUsingEncoding:NSUTF8StringEncoding],
          argv[1],
          [KDECONNECT_DATA_PATH cStringUsingEncoding:NSUTF8StringEncoding],
          initialized_callback
     );
      NSLog(@"Ended OK: %d\n", res);
      exit(res);
    }];

    [thread start];

    CFRunLoopSourceRef powerLoop = IOPSNotificationCreateRunLoopSource(powerSourceCallback, NULL);
    CFRunLoopAddSource(CFRunLoopGetMain(), powerLoop, kCFRunLoopDefaultMode);

    CFRunLoopRun();

    return 0;
  }
}

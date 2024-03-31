#import "MobileGestalt.h"
#import "kdeconnectjb.h"
#import "kern_memorycontrol.h"
#import "rootless.h"
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <unistd.h>

NSString *KDECONNECT_DATA_PATH;

NSString *getDeviceId() {
  NSString *uuid = (__bridge NSString *)MGCopyAnswer(
      kMGUniqueDeviceID, (__bridge CFDictionaryRef)(@{}));
  uuid = [uuid stringByReplacingOccurrencesOfString:@"-" withString:@""];
  uuid = [uuid stringByReplacingOccurrencesOfString:@"_" withString:@""];
  return uuid;
}

int main(int argc, char *argv[], char *envp[]) {
  @autoreleasepool {
    if (argc != 2 && argc != 3) {
      NSLog(@"usage: %s <device_name> [--trollstore]\n", argv[0]);
      return 1;
    }

    if (argc == 3 && strcmp(argv[2], "--trollstore")) {
      NSLog(@"Starting as TrollStore daemon")
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
      NSLog(@"Starting as JB daemon")
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

    bool res = kdeconnect_start(
        [deviceId cStringUsingEncoding:NSUTF8StringEncoding], argv[1],
        [KDECONNECT_DATA_PATH cStringUsingEncoding:NSUTF8StringEncoding]);
    NSLog(@"Ended OK: %d\n", res);
    return 0;
  }
}

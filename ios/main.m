#import <stdio.h>
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import "kdeconnectjb.h"
#import "rootless.h"

NSString *KDECONNECT_DATA_PATH; 

NSString *getDeviceId() {

    NSFileManager *manager = [NSFileManager defaultManager];
    if (![manager createDirectoryAtPath:KDECONNECT_DATA_PATH withIntermediateDirectories:YES attributes:nil error:nil]) {
        printf("Failed to create kdeconnect data dir\n");
        return nil;
    }
    NSString *path = [KDECONNECT_DATA_PATH stringByAppendingPathComponent:@"deviceid"];
    NSData *data = [manager contentsAtPath:path];
    if (data) {
        return [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    } else {
        NSString *uuid = nil;
        while (!uuid) {
            uuid = [[[UIDevice currentDevice] identifierForVendor] UUIDString];
            uuid = [uuid stringByReplacingOccurrencesOfString:@"-" withString:@""];
            uuid = [uuid stringByReplacingOccurrencesOfString:@"_" withString:@""];
        }
        if (![manager createFileAtPath:path contents:[uuid dataUsingEncoding:NSUTF8StringEncoding] attributes:nil]) {
            return nil;
        }
        return uuid;
    }
}

int main(int argc, char *argv[], char *envp[]) {
	@autoreleasepool {
        if (argc != 2) {
            printf("usage: %s <device_name>\n", argv[0]);
            return 1;
        }

        KDECONNECT_DATA_PATH = ROOT_PATH_NS(@"/var/mobile/kdeconnect");
        NSString *deviceId = getDeviceId();
        if (!deviceId) {
            printf("error: No device id\n");
            return 1;
        }

        if (!kdeconnect_init()) {
            printf("error: failed to init kdeconnect");
            return 1;
        }

        bool res = kdeconnect_start(
            [deviceId cStringUsingEncoding:NSUTF8StringEncoding],
            argv[1],
            [KDECONNECT_DATA_PATH cStringUsingEncoding:NSUTF8StringEncoding]
        );
        printf("Ended OK: %d\n", res);
		return 0;
	}
}

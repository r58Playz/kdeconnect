// vim: ft=objc tabstop=2 shiftwidth=2
#import "kdeconnectjb.h"
#import "server.h"

#import <Foundation/Foundation.h>
#import <AppSupport/CPDistributedMessagingCenter.h>

@implementation KConnectServer 
+ (void)load {
	[self sharedInstance];
}

+ (id)sharedInstance {
	static dispatch_once_t once = 0;
	__strong static id sharedInstance = nil;
	dispatch_once(&once, ^{
		sharedInstance = [[self alloc] init];
	});
	return sharedInstance;
}

- (id)init {
	if ((self = [super init])) {
		CPDistributedMessagingCenter * messagingCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.daemon"];
		[messagingCenter runServerOnCurrentThread];

		[messagingCenter registerForMessageName:@"no_longer_lost" target:self selector:@selector(noLongerLost:)];
		[messagingCenter registerForMessageName:@"paired_device_list" target:self selector:@selector(getPairedDeviceList:)];
		[messagingCenter registerForMessageName:@"connected_device_list" target:self selector:@selector(getConnectedDeviceList:)];
    [messagingCenter registerForMessageName:@"connected_device_info" target:self selector:@selector(getConnectedDeviceInfo:userInfo:)];
		[messagingCenter registerForMessageName:@"rebroadcast" target:self selector:@selector(rebroadcast:)];
    NSLog(@"registered CPDistributedMessagingCenter"); 
	}

	return self;
}

- (NSDictionary*)getPairedDeviceList:(NSString *)name {
  NSMutableArray *devices = [NSMutableArray new];
  Vec_KConnectFfiDeviceInfo_t devicesVec = kdeconnect_get_paired_device_list();
  for (int i = 0; i < devicesVec.len; i++) {
    KConnectFfiDeviceInfo_t *deviceInfo = &devicesVec.ptr[i];
    [devices addObject:@{
                @"id": [NSString stringWithUTF8String:deviceInfo->id],
              @"name": [NSString stringWithUTF8String:deviceInfo->name],
              @"type": [NSNumber numberWithInt:deviceInfo->dev_type]
    }];
  }
  kdeconnect_free_paired_device_list(devicesVec);
  return @{@"info": devices};
}

- (NSDictionary*)getConnectedDeviceList:(NSString *)name {
  NSMutableArray *devices = [NSMutableArray new];
  Vec_KConnectFfiDevice_t devicesVec = kdeconnect_get_connected_device_list();
  for (int i = 0; i < devicesVec.len; i++) {
    KConnectFfiDevice_t *device = &devicesVec.ptr[i];
    [devices          addObject:@{
                          @"id": [NSString stringWithUTF8String:device->id],
                        @"name": [NSString stringWithUTF8String:device->name],
                        @"type": [NSNumber numberWithInt:device->dev_type],
               @"battery_level": [NSNumber numberWithInt:kdeconnect_device_get_battery_level(device)],
            @"battery_charging": [NSNumber numberWithBool:kdeconnect_device_get_battery_charging(device)],
     @"battery_under_threshold": [NSNumber numberWithBool:kdeconnect_device_get_battery_under_threshold(device)],
    }];
  }
  kdeconnect_free_connected_device_list(devicesVec);
  return @{@"info": devices};
}

- (NSDictionary*)getConnectedDeviceInfo:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    NSDictionary *deviceInfo = @{
                          @"id": [NSString stringWithUTF8String:device->id],
                        @"name": [NSString stringWithUTF8String:device->name],
                        @"type": [NSNumber numberWithInt:device->dev_type],
               @"battery_level": [NSNumber numberWithInt:kdeconnect_device_get_battery_level(device)],
            @"battery_charging": [NSNumber numberWithBool:kdeconnect_device_get_battery_charging(device)],
     @"battery_under_threshold": [NSNumber numberWithBool:kdeconnect_device_get_battery_under_threshold(device)],
    }; 
    kdeconnect_free_device(device);
    return deviceInfo;
  }
  return @{};
}

- (void)noLongerLost:(NSString *)name {
  kdeconnect_set_is_lost(false);
}

- (void)rebroadcast:(NSString *)name {
  kdeconnect_broadcast_identity();
}
@end

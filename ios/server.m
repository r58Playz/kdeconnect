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

		[messagingCenter registerForMessageName:@"paired_device_list" target:self selector:@selector(getPairedDeviceList:)];
		[messagingCenter registerForMessageName:@"connected_device_list" target:self selector:@selector(getConnectedDeviceList:)];
    [messagingCenter registerForMessageName:@"connected_device_info" target:self selector:@selector(getConnectedDeviceInfo:userInfo:)];
		[messagingCenter registerForMessageName:@"rebroadcast" target:self selector:@selector(rebroadcast:)];
		[messagingCenter registerForMessageName:@"send_ping" target:self selector:@selector(sendPing:userInfo:)];
		[messagingCenter registerForMessageName:@"pair" target:self selector:@selector(pair:userInfo:)];
		[messagingCenter registerForMessageName:@"send_find" target:self selector:@selector(sendFind:userInfo:)];
		[messagingCenter registerForMessageName:@"send_presenter" target:self selector:@selector(sendPresenter:userInfo:)];
		[messagingCenter registerForMessageName:@"stop_presenter" target:self selector:@selector(sendPresenterStop:userInfo:)];
		[messagingCenter registerForMessageName:@"killyourself" target:self selector:@selector(kill:)];
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
                      @"paired": [NSNumber numberWithBool:kdeconnect_device_is_paired(device)],
               @"battery_level": [NSNumber numberWithInt:kdeconnect_device_get_battery_level(device)],
            @"battery_charging": [NSNumber numberWithBool:kdeconnect_device_get_battery_charging(device)],
     @"battery_under_threshold": [NSNumber numberWithBool:kdeconnect_device_get_battery_under_threshold(device)],
                   @"clipboard": [NSString stringWithUTF8String:kdeconnect_device_get_clipboard_content(device)],
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
                      @"paired": [NSNumber numberWithBool:kdeconnect_device_is_paired(device)],
               @"battery_level": [NSNumber numberWithInt:kdeconnect_device_get_battery_level(device)],
            @"battery_charging": [NSNumber numberWithBool:kdeconnect_device_get_battery_charging(device)],
     @"battery_under_threshold": [NSNumber numberWithBool:kdeconnect_device_get_battery_under_threshold(device)],
                   @"clipboard": [NSString stringWithUTF8String:kdeconnect_device_get_clipboard_content(device)],
    }; 
    kdeconnect_free_device(device);
    return deviceInfo;
  }
  return @{};
}

- (void)rebroadcast:(NSString *)name {
  kdeconnect_broadcast_identity();
}

- (void)sendPing:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_send_ping(device);
    kdeconnect_free_device(device);
  }
}
- (void) pair:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  NSNumber *pairStatus = (NSNumber*)[userInfo objectForKey:@"pair"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_pair(device, [pairStatus boolValue]);
    kdeconnect_free_device(device);
  }
}
- (void)sendFind:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_send_find(device);
    kdeconnect_free_device(device);
  }
}
- (void)sendPresenter:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  NSNumber *dx = (NSNumber*)[userInfo objectForKey:@"dx"];
  NSNumber *dy = (NSNumber*)[userInfo objectForKey:@"dy"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_send_presenter(device, [dx floatValue], [dy floatValue]);
    kdeconnect_free_device(device);
  }
}
- (void)sendPresenterStop:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_stop_presenter(device);
    kdeconnect_free_device(device);
  }
}
- (void)kill:(NSString *)name {
  exit(0);
}
@end

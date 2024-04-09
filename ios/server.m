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

- (void)noLongerLost:(NSString *)name {
  kdeconnect_set_is_lost(false);
}
@end

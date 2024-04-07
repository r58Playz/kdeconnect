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
    NSLog(@"registered CPDistributedMessagingCenter"); 
	}

	return self;
}

- (void)noLongerLost:(NSString *)name {
  kdeconnect_set_is_lost(false);
}
@end

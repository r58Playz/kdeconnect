// vim: ft=objc tabstop=2 shiftwidth=2
#import <Foundation/Foundation.h>
#import <AppSupport/CPDistributedMessagingCenter.h>
#import <UIKit/UIAlertController.h>

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
#import <SpringBoardUI/SBAlertItem.h>
#pragma clang diagnostic pop

CPDistributedMessagingCenter *daemonMessageCenter;

@interface KConnectAlert : SBAlertItem 
@property (nonatomic, strong) NSString *title;
@property (nonatomic, strong) NSString *message;
@property (nonatomic, strong) NSString *actionTitle;
@property (nonatomic, copy) void (^block)(void);
@end

@implementation KConnectAlert
- (id)initWithTitle:(NSString*)title message:(NSString*)message actionTitle:(NSString*)actionTitle block:(void (^)())block {
  self = [self init];
  self.title = title;
  self.message = message;
  self.actionTitle = actionTitle;
  self.block = block;
  return self;
}
- (void)configure:(BOOL)arg1 requirePasscodeForActions:(BOOL)arg2 {
  UIAlertController *alertController = [self alertController];
  alertController.title = self.title;
  alertController.message = self.message;

  UIAlertAction *confirmAction = [UIAlertAction actionWithTitle:self.actionTitle
                                                          style:UIAlertActionStyleDefault
                                                        handler:^(UIAlertAction * _Nonnull action) {
    self.block();
    [self dismiss];
  }];

  [alertController addAction:confirmAction];
}

-(BOOL)reappearsAfterUnlock {
  return YES;
}
@end

@interface KConnectTweakServer : NSObject
+(void)load;
@end

@implementation KConnectTweakServer 
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
		CPDistributedMessagingCenter * messagingCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.springboard"];
		[messagingCenter runServerOnCurrentThread];

    [messagingCenter registerForMessageName:@"lost" target:self selector:@selector(lost:)];
    [messagingCenter registerForMessageName:@"ping" target:self selector:@selector(ping:withInfo:)];
    NSLog(@"registered CPDistributedMessagingCenter"); 
	}

	return self;
}

- (void)lost:(NSString *)lost {
  SBAlertItem *item = [[KConnectAlert alloc] initWithTitle:@"KDE Connect"
                                                       message:@"Ring My Phone Alert"
                                                   actionTitle:@"Stop"
                                                         block: ^() {
    NSLog(@"confirmed no longer lost!");
    [daemonMessageCenter sendMessageName:@"no_longer_lost" userInfo:nil];
  }];
  [KConnectAlert activateAlertItem: item];
}

- (void)ping:(NSString*)ping withInfo:(NSDictionary*)info {
  NSString *name = (NSString*)[info objectForKey:@"name"];
  if (name) {
    SBAlertItem *item = [[KConnectAlert alloc] initWithTitle:@"KDE Connect"
                                                         message:[@"Recieved ping from device: " stringByAppendingString:name]
                                                     actionTitle:@"OK"
                                                           block: ^() {}];
    [KConnectAlert activateAlertItem: item];
  }
}
@end

%ctor {
  daemonMessageCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.daemon"];
  [KConnectTweakServer load];
}

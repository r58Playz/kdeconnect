// vim: ft=objc tabstop=2 shiftwidth=2
#import <Foundation/Foundation.h>
#import <mach-o/dyld.h>
#import <spawn.h>
#import <sys/sysctl.h>

int spawnRoot(NSString *path, NSArray *args);

void createMessageCenter();
NSArray *getConnectedDevices();
NSArray *getPairedDevices();

NSArray *sysctl_ps(void);

@class KConnectSwiftServer;

@interface KConnectObjcServer : NSObject
@property (nonatomic, strong) KConnectSwiftServer *swift;
+(id)newWithSwift:(KConnectSwiftServer*)swift;
@end


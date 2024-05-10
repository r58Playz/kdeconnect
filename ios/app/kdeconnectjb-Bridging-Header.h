// vim: ft=objc tabstop=2 shiftwidth=2
#import <Foundation/Foundation.h>
#import <mach-o/dyld.h>
#import <spawn.h>
#import <sys/sysctl.h>

int spawnRoot(NSString *path, NSArray *args);

void createMessageCenter();
NSArray *getConnectedDevices();
NSArray *getPairedDevices();
void rebroadcast();
void sendPing(NSString *id);
void sendPairReq(NSString *id, NSNumber *pair);
void sendFind(NSString *id);
void sendPresenter(NSString *id, NSNumber *dx, NSNumber *dy);
void stopPresenter(NSString *id);
void requestVolume(NSString *id);
void sendVolume(NSString *id, NSString *name, NSNumber *enabled, NSNumber *muted, NSNumber *volume);
void sendFiles(NSString *id, NSArray *files, NSNumber* open);
void sendExit();

@class KConnectSwiftServer;

@interface KConnectObjcServer : NSObject
@property (nonatomic, strong) KConnectSwiftServer *swift;
+(id)newWithSwift:(KConnectSwiftServer*)swift;
@end


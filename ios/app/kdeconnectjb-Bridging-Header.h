#import <Foundation/Foundation.h>
#import <mach-o/dyld.h>
#import <spawn.h>
#import <sys/sysctl.h>

int spawnRoot(NSString *path, NSArray *args);

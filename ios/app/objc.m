// vim: tabstop=2 shiftwidth=2
#import <AppSupport/CPDistributedMessagingCenter.h>
#import <Foundation/Foundation.h>
#import <mach-o/dyld.h>
#import <spawn.h>
#import <sys/sysctl.h>
#import <kdeconnectjb-Swift.h>

CPDistributedMessagingCenter *daemonMessageCenter;

#define POSIX_SPAWN_PERSONA_FLAGS_OVERRIDE 1
extern int posix_spawnattr_set_persona_np(const posix_spawnattr_t *__restrict,
                                          uid_t, uint32_t);
extern int
posix_spawnattr_set_persona_uid_np(const posix_spawnattr_t *__restrict, uid_t);
extern int
posix_spawnattr_set_persona_gid_np(const posix_spawnattr_t *__restrict, uid_t);

int spawnRoot(NSString *path, NSArray *args) {
  NSMutableArray *argsM = args.mutableCopy ?: [NSMutableArray new];
  [argsM insertObject:path atIndex:0];

  NSUInteger argCount = [argsM count];
  char **argsC = (char **)malloc((argCount + 1) * sizeof(char *));

  for (NSUInteger i = 0; i < argCount; i++) {
    argsC[i] = strdup([[argsM objectAtIndex:i] UTF8String]);
  }
  argsC[argCount] = NULL;

  posix_spawnattr_t attr;
  posix_spawnattr_init(&attr);

  posix_spawnattr_set_persona_np(&attr, 99, POSIX_SPAWN_PERSONA_FLAGS_OVERRIDE);
  posix_spawnattr_set_persona_uid_np(&attr, 0);
  posix_spawnattr_set_persona_gid_np(&attr, 0);

  posix_spawn_file_actions_t action;
  posix_spawn_file_actions_init(&action);

  pid_t task_pid;
  int spawnError = posix_spawn(&task_pid, [path UTF8String], &action, &attr,
                               (char *const *)argsC, NULL);
  posix_spawnattr_destroy(&attr);

  if (spawnError != 0) {
    NSLog(@"posix_spawn error %d\n", spawnError);
    return spawnError;
  }

  return 0;
}

void createMessageCenter() {
  daemonMessageCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.daemon"];
}

NSArray *getConnectedDevices() {
    return [[daemonMessageCenter sendMessageAndReceiveReplyName:@"connected_device_list" userInfo:nil] objectForKey:@"info"];
}

NSArray *getPairedDevices() {
    return [[daemonMessageCenter sendMessageAndReceiveReplyName:@"paired_device_list" userInfo:nil] objectForKey:@"info"];
}

@interface KConnectObjcServer : NSObject
@property (nonatomic, strong) KConnectSwiftServer *swift;
@end

@implementation KConnectObjcServer 
+ (id)newWithSwift:(KConnectSwiftServer*) swift {
  return [[self alloc] initWithSwift:swift];
}
- (id)initWithSwift:(KConnectSwiftServer*) swift {
	if ((self = [super init])) {
    self.swift = swift;
		CPDistributedMessagingCenter * messagingCenter = [CPDistributedMessagingCenter centerNamed:@"dev.r58playz.kdeconnectjb.app"];
		[messagingCenter runServerOnCurrentThread];

    [messagingCenter registerForMessageName:@"refresh" target:self selector:@selector(refresh:)];
    NSLog(@"registered CPDistributedMessagingCenter"); 
	}

	return self;
}
- (void)refresh:(NSString*)refresh {
  [self.swift refreshRequested];
}
@end

#import <sys/sysctl.h>
#define PROC_PIDPATHINFO                11
#define PROC_PIDPATHINFO_SIZE           (MAXPATHLEN)
#define PROC_PIDPATHINFO_MAXSIZE        (4 * MAXPATHLEN)
#define PROC_ALL_PIDS                    1

int proc_pidpath(int pid, void *buffer, uint32_t buffersize);
int proc_listpids(uint32_t type, uint32_t typeinfo, void *buffer, int buffersize);

NSArray *sysctl_ps(void) {
    NSMutableArray *array = [[NSMutableArray alloc] init];

    int numberOfProcesses = proc_listpids(PROC_ALL_PIDS, 0, NULL, 0);
    pid_t pids[numberOfProcesses];
    bzero(pids, sizeof(pids));
    proc_listpids(PROC_ALL_PIDS, 0, pids, sizeof(pids));
    for (int i = 0; i < numberOfProcesses; ++i) {
        if (pids[i] == 0) { continue; }
        char pathBuffer[PROC_PIDPATHINFO_MAXSIZE];
        bzero(pathBuffer, PROC_PIDPATHINFO_MAXSIZE);
        proc_pidpath(pids[i], pathBuffer, sizeof(pathBuffer));

        if (strlen(pathBuffer) > 0) {
            NSString *processID = [[NSString alloc] initWithFormat:@"%d", pids[i]];
            NSString *processName = [[NSString stringWithUTF8String:pathBuffer] lastPathComponent];
            NSDictionary *dict = [[NSDictionary alloc] initWithObjects:[NSArray arrayWithObjects:processID, processName, nil] forKeys:[NSArray arrayWithObjects:@"pid", @"proc_name", nil]];

            [array addObject:dict];
        }
    }

    return [array copy];
}

// vim: tabstop=2 shiftwidth=2
#import <AppSupport/CPDistributedMessagingCenter.h>
#import <Foundation/Foundation.h>
#import <mach-o/dyld.h>
#import <spawn.h>
#import <sys/sysctl.h>

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

// vim: ft=objc tabstop=2 shiftwidth=2
#import "kdeconnectjb.h"
#import "server.h"

#import <Foundation/Foundation.h>
#import <AppSupport/CPDistributedMessagingCenter.h>

NSDictionary *getDeviceInfo(KConnectFfiDevice_t *device) {
  NSMutableArray *connectivity = [NSMutableArray new];
  Vec_KConnectConnectivitySignal_t connectivitySignals = kdeconnect_device_get_connectivity_report(device);
  for (int i = 0; i < connectivitySignals.len; i++) {
    KConnectConnectivitySignal_t *signal = &connectivitySignals.ptr[i];
    [connectivity addObject:@{
                      @"id": [NSString stringWithUTF8String:signal->id],
                    @"type": [NSString stringWithUTF8String:signal->signal_type],
                @"strength": [NSNumber numberWithInt:signal->strength],
    }];
  }
  kdeconnect_free_connectivity_report(connectivitySignals);

  NSMutableArray *volume = [NSMutableArray new];
  Vec_KConnectVolumeStream_t volumeStreams = kdeconnect_device_get_volume(device);
  for (int i = 0; i < volumeStreams.len; i++) {
    KConnectVolumeStream_t *stream = &volumeStreams.ptr[i];
    [volume addObject:@{
              @"name": [NSString stringWithUTF8String:stream->name],
       @"description": [NSString stringWithUTF8String:stream->description],
       @"has_enabled": [NSNumber numberWithBool:stream->has_enabled],
           @"enabled": [NSNumber numberWithBool:stream->enabled],
             @"muted": [NSNumber numberWithBool:stream->muted],
        @"max_volume": [NSNumber numberWithInt:stream->has_max_volume ? stream->max_volume : 100],
            @"volume": [NSNumber numberWithInt:stream->volume],
    }];
  }
  kdeconnect_free_volume(volumeStreams);

	NSMutableArray *player = [NSMutableArray new];
	Vec_KConnectMprisPlayer_t players = kdeconnect_device_get_players(device);
	for (int i = 0; i < players.len; i++) {
		KConnectMprisPlayer_t *mprisPlayer = &players.ptr[i];
		[player     addObject:@{
										@"id": [NSString stringWithUTF8String:mprisPlayer->player],
								 @"title": [NSString stringWithUTF8String:mprisPlayer->title],
								@"artist": [NSString stringWithUTF8String:mprisPlayer->artist],
								 @"album": [NSString stringWithUTF8String:mprisPlayer->album],
						 @"album_art": [NSString stringWithUTF8String:mprisPlayer->album_art_url],
									 @"url": [NSString stringWithUTF8String:mprisPlayer->url],
						@"is_playing": [NSNumber numberWithBool:mprisPlayer->is_playing],
						 @"can_pause": [NSNumber numberWithBool:mprisPlayer->can_pause],
							@"can_play": [NSNumber numberWithBool:mprisPlayer->can_play],
					 @"can_go_next": [NSNumber numberWithBool:mprisPlayer->can_go_next],
			 @"can_go_previous": [NSNumber numberWithBool:mprisPlayer->can_go_previous],
							@"can_seek": [NSNumber numberWithBool:mprisPlayer->can_seek],
							 @"shuffle": [NSNumber numberWithBool:mprisPlayer->shuffle],
							@"position": [NSNumber numberWithInt:mprisPlayer->pos],
								@"length": [NSNumber numberWithInt:mprisPlayer->length],
								@"volume": [NSNumber numberWithInt:mprisPlayer->volume],
									@"loop": [NSNumber numberWithInt:mprisPlayer->loop_status],
		}];
	}
	kdeconnect_free_players(players);

	NSMutableArray *commandArr = [NSMutableArray new];
	Vec_KConnectCommand_t commands = kdeconnect_device_get_commands(device);
	for (int i = 0; i < commands.len; i++) {
		KConnectCommand_t *command = &commands.ptr[i];
		[commandArr addObject:@{
										@"id": [NSString stringWithUTF8String:command->id],
									@"name": [NSString stringWithUTF8String:command->name],
							 @"command": [NSString stringWithUTF8String:command->command],
		}];
	}

  return @{
                          @"id": [NSString stringWithUTF8String:device->id],
                        @"name": [NSString stringWithUTF8String:device->name],
                        @"type": [NSNumber numberWithInt:device->dev_type],
                      @"paired": [NSNumber numberWithBool:kdeconnect_device_is_paired(device)],
               @"battery_level": [NSNumber numberWithInt:kdeconnect_device_get_battery_level(device)],
            @"battery_charging": [NSNumber numberWithBool:kdeconnect_device_get_battery_charging(device)],
     @"battery_under_threshold": [NSNumber numberWithBool:kdeconnect_device_get_battery_under_threshold(device)],
                   @"clipboard": [NSString stringWithUTF8String:kdeconnect_device_get_clipboard_content(device)],
                @"connectivity": connectivity,
                      @"volume": volume,
											@"player": player,
										 @"command": commandArr,
  };
}

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
		[messagingCenter registerForMessageName:@"rebroadcast" target:self selector:@selector(rebroadcast:)];
		[messagingCenter registerForMessageName:@"send_ping" target:self selector:@selector(sendPing:userInfo:)];
		[messagingCenter registerForMessageName:@"pair" target:self selector:@selector(pair:userInfo:)];
		[messagingCenter registerForMessageName:@"send_find" target:self selector:@selector(sendFind:userInfo:)];
		[messagingCenter registerForMessageName:@"send_presenter" target:self selector:@selector(sendPresenter:userInfo:)];
		[messagingCenter registerForMessageName:@"stop_presenter" target:self selector:@selector(sendPresenterStop:userInfo:)];
		[messagingCenter registerForMessageName:@"get_volume" target:self selector:@selector(requestVolume:userInfo:)];
		[messagingCenter registerForMessageName:@"send_volume" target:self selector:@selector(sendVolume:userInfo:)];
		[messagingCenter registerForMessageName:@"share_files" target:self selector:@selector(shareFiles:userInfo:)];
		[messagingCenter registerForMessageName:@"get_players" target:self selector:@selector(requestPlayers:userInfo:)];
		[messagingCenter registerForMessageName:@"request_player_action" target:self selector:@selector(requestPlayerAction:userInfo:)];
		[messagingCenter registerForMessageName:@"request_mousepad_action" target:self selector:@selector(requestMousepad:userInfo:)];
		[messagingCenter registerForMessageName:@"get_commands" target:self selector:@selector(requestCommands:userInfo:)];
		[messagingCenter registerForMessageName:@"run_command" target:self selector:@selector(runCommand:userInfo:)];
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
    [devices addObject:getDeviceInfo(&devicesVec.ptr[i])];
  }
  kdeconnect_free_connected_device_list(devicesVec);
  return @{@"info": devices};
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
- (void)pair:(NSString *)name userInfo:(NSDictionary*)userInfo {
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
- (void)requestVolume:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_request_volume(device);
    kdeconnect_free_device(device);
  }
}
- (void)sendVolume:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  NSString *streamName = (NSString*)[userInfo objectForKey:@"name"];
  NSNumber *enabled = (NSNumber*)[userInfo objectForKey:@"enabled"];
  NSNumber *muted = (NSNumber*)[userInfo objectForKey:@"muted"];
  NSNumber *volume = (NSNumber*)[userInfo objectForKey:@"volume"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_send_volume_update(device, [streamName UTF8String], [enabled boolValue], [muted boolValue], [volume intValue]);
    kdeconnect_free_device(device);
  }
}
- (void)shareFiles:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  NSArray *files = (NSArray*)[userInfo objectForKey:@"files"];
  NSNumber *open = (NSNumber*)[userInfo objectForKey:@"open"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device && files.count) {
    if (files.count == 1) {
      kdeconnect_device_share_file(device, ((NSString*)files.firstObject).UTF8String, [open boolValue]);
    } else {
      int cnt = files.count;
      char** filesArr = calloc(cnt, sizeof(char*));

      for (int i = 0; i < cnt; i++) {
        filesArr[i] = (char*)[[files objectAtIndex:i] UTF8String];
      }

      slice_ref_char_const_ptr_t arr = { .ptr = (char const * const *)filesArr, .len = cnt };

      kdeconnect_device_share_files(device, arr, [open boolValue]);

      free(filesArr);
    }
    kdeconnect_free_device(device);
  }
}
- (void)requestPlayers:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_request_players(device);
    kdeconnect_free_device(device);
  }
}
- (void)requestPlayer:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  NSString *playerid = (NSString*)[userInfo objectForKey:@"player_id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_request_player(device, [playerid UTF8String]);
    kdeconnect_free_device(device);
  }
}
- (void)requestPlayerAction:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  NSString *playerid = (NSString*)[userInfo objectForKey:@"player_id"];
	NSNumber *playeraction = (NSNumber*)[userInfo objectForKey:@"player_action"];
	NSNumber *playeractionint = (NSNumber*)[userInfo objectForKey:@"player_action_int"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_request_player_action(device, [playerid UTF8String], [playeraction intValue], [playeractionint intValue]);
    kdeconnect_free_device(device);
  }
}
- (void)requestMousepad:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
	NSString *key = (NSString*)[userInfo objectForKey:@"key"];
	NSNumber *special_key = (NSNumber*)[userInfo objectForKey:@"special_key"];
	NSNumber *alt = (NSNumber*)[userInfo objectForKey:@"alt"];
	NSNumber *ctrl = (NSNumber*)[userInfo objectForKey:@"ctrl"];
	NSNumber *shift = (NSNumber*)[userInfo objectForKey:@"shift"];
	NSNumber *dx = (NSNumber*)[userInfo objectForKey:@"dx"];
	NSNumber *dy = (NSNumber*)[userInfo objectForKey:@"dy"];
	NSNumber *scroll = (NSNumber*)[userInfo objectForKey:@"scroll"];
	NSNumber *singleclick = (NSNumber*)[userInfo objectForKey:@"singleclick"];
	NSNumber *doubleclick = (NSNumber*)[userInfo objectForKey:@"doubleclick"];
	NSNumber *middleclick = (NSNumber*)[userInfo objectForKey:@"middleclick"];
	NSNumber *rightclick = (NSNumber*)[userInfo objectForKey:@"rightclick"];
	NSNumber *singlehold = (NSNumber*)[userInfo objectForKey:@"singlehold"];
	NSNumber *singlerelease = (NSNumber*)[userInfo objectForKey:@"singlerelease"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
		KConnectMousepadRequest_t request = {
			.key = [key UTF8String],
			.special_key = [special_key intValue],
			.alt = [alt boolValue],
			.ctrl = [ctrl boolValue],
			.shift = [shift boolValue],
			.dx = [dx floatValue],
			.dy = [dy floatValue],
			.scroll = [scroll boolValue],
			.singleclick = [singleclick boolValue],
			.doubleclick = [doubleclick boolValue],
			.middleclick = [middleclick boolValue],
			.rightclick = [rightclick boolValue],
			.singlehold = [singlehold boolValue],
			.singlerelease = [singlerelease boolValue],
			.send_ack = false,
		};
    kdeconnect_device_request_mousepad(device, request);
    kdeconnect_free_device(device);
  }
}
- (void)requestCommands:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_request_commands(device);
    kdeconnect_free_device(device);
  }
}
- (void)runCommand:(NSString *)name userInfo:(NSDictionary*)userInfo {
  NSString *id = (NSString*)[userInfo objectForKey:@"id"];
	NSString *commandId = (NSString*)[userInfo objectForKey:@"command_id"];
  KConnectFfiDevice_t *device = kdeconnect_get_device_by_id([id UTF8String]);
  if (device) {
    kdeconnect_device_run_command(device, [commandId UTF8String]);
    kdeconnect_free_device(device);
  }
}
- (void)kill:(NSString *)name {
  exit(0);
}
@end

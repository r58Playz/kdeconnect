/*
 * Copyright (c) 2002 Apple Computer, Inc. All rights reserved.
 *
 * @APPLE_LICENSE_HEADER_START@
 *
 * This file contains Original Code and/or Modifications of Original Code
 * as defined in and that are subject to the Apple Public Source License
 * Version 2.0 (the 'License'). You may not use this file except in
 * compliance with the License. Please obtain a copy of the License at
 * http://www.opensource.apple.com/apsl/ and read it before using this
 * file.
 *
 * The Original Code and all software distributed under the License are
 * distributed on an 'AS IS' basis, WITHOUT WARRANTY OF ANY KIND, EITHER
 * EXPRESS OR IMPLIED, AND APPLE HEREBY DISCLAIMS ALL SUCH WARRANTIES,
 * INCLUDING WITHOUT LIMITATION, ANY WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE, QUIET ENJOYMENT OR NON-INFRINGEMENT.
 * Please see the License for the specific language governing rights and
 * limitations under the License.
 *
 * @APPLE_LICENSE_HEADER_END@
 */

#import <Foundation/Foundation.h>
#import <CoreFoundation/CoreFoundation.h>

#ifndef _IOKIT_IOPOWERSOURCES_H
#define _IOKIT_IOPOWERSOURCES_H

#include <sys/cdefs.h>
__BEGIN_DECLS

#define kIOPSNotifyLowBattery   "com.apple.system.powersources.lowbattery"

typedef enum {
    kIOPSLowBatteryWarningNone  = 1,

    kIOPSLowBatteryWarningEarly = 2,

    kIOPSLowBatteryWarningFinal = 3
} IOPSLowBatteryWarningLevel;

IOPSLowBatteryWarningLevel IOPSGetBatteryWarningLevel(void);

#define kIOPSTimeRemainingNotificationKey        "com.apple.system.powersources.timeremaining"

#define     kIOPSTimeRemainingUnknown           ((CFTimeInterval)-1.0)

#define     kIOPSTimeRemainingUnlimited         ((CFTimeInterval)-2.0)

CFTimeInterval IOPSGetTimeRemainingEstimate(void);

typedef void  (*IOPowerSourceCallbackType)(void *context);

CFTypeRef IOPSCopyPowerSourcesInfo(void);

CFStringRef     IOPSGetProvidingPowerSourceType(CFTypeRef snapshot);

CFArrayRef IOPSCopyPowerSourcesList(CFTypeRef blob);

CFDictionaryRef IOPSGetPowerSourceDescription(CFTypeRef blob, CFTypeRef ps);

CFRunLoopSourceRef IOPSNotificationCreateRunLoopSource(IOPowerSourceCallbackType callback, void *context);

CFDictionaryRef IOPSCopyExternalPowerAdapterDetails(void);

__END_DECLS

#endif 

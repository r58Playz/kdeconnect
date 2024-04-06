/*
 * Copyright (c) 2002-2010 Apple Computer, Inc. All rights reserved.
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

#ifndef _IOPSKEYS_H_
#define _IOPSKEYS_H_
#define kIOPSPowerAdapterIDKey          "AdapterID"
#define kIOPSPowerAdapterWattsKey       "Watts"
#define kIOPSPowerAdapterRevisionKey   "AdapterRevision"
#define kIOPSPowerAdapterSerialNumberKey    "SerialNumber"
#define kIOPSPowerAdapterFamilyKey          "FamilyCode"

#define kIOPSUPSManagementClaimed       "/IOKit/UPSPowerManagementClaimed"
#define kIOPSLowWarnLevelKey           "Low Warn Level"
#define kIOPSDeadWarnLevelKey          "Shutdown Level"

#define kIOPSDynamicStorePath          "/IOKit/PowerSources"

#define kIOPSCommandDelayedRemovePowerKey     "Delayed Remove Power"
#define kIOPSCommandEnableAudibleAlarmKey     "Enable Audible Alarm"
#define kIOPSCommandStartupDelayKey           "Startup Delay"

#define kIOPSPowerSourceIDKey          "Power Source ID"
#define kIOPSPowerSourceStateKey       "Power Source State"
#define kIOPSCurrentCapacityKey        "Current Capacity"
#define kIOPSMaxCapacityKey            "Max Capacity"
#define kIOPSDesignCapacityKey          "DesignCapacity"
#define kIOPSTimeToEmptyKey            "Time to Empty"
#define kIOPSTimeToFullChargeKey       "Time to Full Charge"
#define kIOPSIsChargingKey             "Is Charging"
#define kIOPSIsPresentKey              "Is Present"
#define kIOPSVoltageKey                "Voltage"
#define kIOPSCurrentKey                "Current"
#define kIOPSNameKey                   "Name"
#define kIOPSTypeKey          "Type"
#define kIOPSTransportTypeKey          "Transport Type"
#define kIOPSVendorDataKey          "Vendor Specific Data"
#define kIOPSBatteryHealthKey       "BatteryHealth"
#define kIOPSBatteryHealthConditionKey       "BatteryHealthCondition"
#define kIOPSBatteryFailureModesKey          "BatteryFailureModes"
#define kIOPSHealthConfidenceKey    "HealthConfidence"
#define kIOPSMaxErrKey              "MaxErr"
#define kIOPSIsChargedKey                   "Is Charged"
#define kIOPSIsFinishingChargeKey              "Is Finishing Charge"
#define kIOPSHardwareSerialNumberKey            "Hardware Serial Number"

#define kIOPSSerialTransportType       "Serial"
#define kIOPSUSBTransportType          "USB"
#define kIOPSNetworkTransportType      "Ethernet"
#define kIOPSInternalType              "Internal"

#define kIOPSInternalBatteryType    "InternalBattery"
#define kIOPSUPSType                "UPS"
#define kIOPSOffLineValue              "Off Line"
#define kIOPSACPowerValue              "AC Power"
#define kIOPSBatteryPowerValue         "Battery Power"

#define kIOPSPoorValue                  "Poor"
#define kIOPSFairValue                  "Fair"
#define kIOPSGoodValue                  "Good"

#define kIOPSCheckBatteryValue                      "Check Battery"
#define kIOPSPermanentFailureValue                  "Permanent Battery Failure"

#define kIOPSFailureExternalInput                   "Externally Indicated Failure"
#define kIOPSFailureSafetyOverVoltage               "Safety Over-Voltage"
#define kIOPSFailureChargeOverTemp                  "Charge Over-Temperature"
#define kIOPSFailureDischargeOverTemp               "Discharge Over-Temperature"
#define kIOPSFailureCellImbalance                   "Cell Imbalance"
#define kIOPSFailureChargeFET                       "Charge FET"
#define kIOPSFailureDischargeFET                    "Discharge FET"
#define kIOPSFailureDataFlushFault                  "Data Flush Fault"
#define kIOPSFailurePermanentAFEComms               "Permanent AFE Comms"
#define kIOPSFailurePeriodicAFEComms                "Periodic AFE Comms"
#define kIOPSFailureChargeOverCurrent               "Charge Over-Current"
#define kIOPSFailureDischargeOverCurrent            "Discharge Over-Current"
#define kIOPSFailureOpenThermistor                  "Open Thermistor"
#define kIOPSFailureFuseBlown                       "Fuse Blown"

#endif

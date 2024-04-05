TARGET := iphone:clang:latest:15.0

ifeq ($(FINALPACKAGE),1)
$(shell cd ../kdeconnectjb; cargo b -r --target aarch64-apple-ios || exit 1)
else
$(shell cd ../kdeconnectjb; cargo b --target aarch64-apple-ios || exit 1)
endif

ARCHS = arm64

include $(THEOS)/makefiles/common.mk

TOOL_NAME = kdeconnectjb

kdeconnectjb_FILES = main.m
kdeconnectjb_CFLAGS = -fobjc-arc -I../kdeconnectjb/target/
kdeconnectjb_CODESIGN_FLAGS = -Sentitlements.plist
kdeconnectjb_INSTALL_PATH = /usr/local/bin
kdeconnectjb_LD_FLAGS = -lkdeconnectjb
kdeconnectjb_LIBRARIES = MobileGestalt
kdeconnectjb_FRAMEWORKS = Foundation CoreFoundation MobileCoreServices CoreServices CallKit IOKit
kdeconnectjb_PRIVATE_FRAMEWORKS = AppSupport CoreTelephony
ifeq ($(FINALPACKAGE),1)
kdeconnectjb_OBJ_FILES = ../target/aarch64-apple-ios/release/libkdeconnectjb.a
else
kdeconnectjb_OBJ_FILES = ../target/aarch64-apple-ios/debug/libkdeconnectjb.a
endif

include $(THEOS_MAKE_PATH)/tool.mk
SUBPROJECTS += app 
include $(THEOS_MAKE_PATH)/aggregate.mk

TARGET := iphone:clang:latest:15.0

ifeq ($(FINALPACKAGE),1)
$(shell cd ../kdeconnectjb; cargo b -r --target aarch64-apple-ios || exit 1)
else
$(shell cd ../kdeconnectjb; cargo b --target aarch64-apple-ios || exit 1)
endif

$(shell cd ../kdeconnectjb; cargo r --bin generate-headers --features headers -- ../target/kdeconnectjb.h)

ARCHS = arm64

include $(THEOS)/makefiles/common.mk

TOOL_NAME = kdeconnectd

kdeconnectd_FILES = $(wildcard *.m) $(wildcard *.swift)
kdeconnectd_CFLAGS = -fobjc-arc -I../target/
kdeconnectd_CODESIGN_FLAGS = -Sentitlements.plist
kdeconnectd_INSTALL_PATH = /usr/local/bin
kdeconnectd_LD_FLAGS = -lkdeconnectjb
kdeconnectd_LIBRARIES = MobileGestalt
kdeconnectd_FRAMEWORKS = Foundation CoreFoundation MobileCoreServices CoreServices CallKit IOKit
kdeconnectd_PRIVATE_FRAMEWORKS = AppSupport CoreTelephony
ifeq ($(FINALPACKAGE),1)
kdeconnectd_OBJ_FILES = ../target/aarch64-apple-ios/release/libkdeconnectjb.a
else
kdeconnectd_OBJ_FILES = ../target/aarch64-apple-ios/debug/libkdeconnectjb.a
endif

include $(THEOS_MAKE_PATH)/tool.mk
SUBPROJECTS += app

include $(THEOS_MAKE_PATH)/aggregate.mk

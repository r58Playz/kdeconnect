TARGET := iphone:clang:latest:15.0
INSTALL_TARGET_PROCESSES = kdeconnectd

ARCHS = arm64

include $(THEOS)/makefiles/common.mk

TOOL_NAME = kdeconnectd

kdeconnectd_FILES = $(wildcard *.m) $(wildcard *.swift)
kdeconnectd_CFLAGS = -fobjc-arc -I../target/ -I.
kdeconnectd_CODESIGN_FLAGS = -Sentitlements.plist
kdeconnectd_INSTALL_PATH = /usr/local/bin
kdeconnectd_LD_FLAGS = -lkdeconnectjb
kdeconnectd_LIBRARIES = MobileGestalt
kdeconnectd_FRAMEWORKS = Foundation CoreFoundation MobileCoreServices CoreServices CallKit IOKit AudioToolbox
kdeconnectd_PRIVATE_FRAMEWORKS = AppSupport CoreTelephony MobileWiFi Pasteboard MediaPlayer FrontBoardServices SpringBoardServices
ifeq ($(FINALPACKAGE),1)
kdeconnectd_OBJ_FILES = ../target/aarch64-apple-ios/release/libkdeconnectjb.a
else
kdeconnectd_OBJ_FILES = ../target/aarch64-apple-ios/debug/libkdeconnectjb.a
endif

include $(THEOS_MAKE_PATH)/tool.mk
SUBPROJECTS += app

include $(THEOS_MAKE_PATH)/aggregate.mk

internal-package::
ifeq ($(PACKAGE_FORMAT),ipa)
	cp .theos/_/usr/local/bin/kdeconnectd .theos/_/Payload/kdeconnectjb.app/
endif

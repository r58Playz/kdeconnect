TARGET := iphone:clang:latest:15.0

ifeq ($(FINALPACKAGE),1)
$(shell cd rust; cargo b -r --target aarch64-apple-ios)
else
$(shell cd rust; cargo b --target aarch64-apple-ios)
endif

ARCHS = arm64

include $(THEOS)/makefiles/common.mk

TOOL_NAME = kdeconnectjb

kdeconnectjb_FILES = main.m
kdeconnectjb_CFLAGS = -fobjc-arc -Irust/target/
kdeconnectjb_CODESIGN_FLAGS = -Sentitlements.plist
kdeconnectjb_INSTALL_PATH = /usr/local/bin
kdeconnectjb_LD_FLAGS = -lkdeconnectjb
ifeq ($(FINALPACKAGE),1)
kdeconnectjb_OBJ_FILES = rust/target/aarch64-apple-ios/release/libkdeconnectjb.a
else
kdeconnectjb_OBJ_FILES = rust/target/aarch64-apple-ios/debug/libkdeconnectjb.a
endif

include $(THEOS_MAKE_PATH)/tool.mk

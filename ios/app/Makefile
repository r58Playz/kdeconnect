TARGET := iphone:clang:latest:15.0
INSTALL_TARGET_PROCESSES = kdeconnectjb

ARCHS = arm64

include $(THEOS)/makefiles/common.mk

APPLICATION_NAME = kdeconnectjb

kdeconnectjb_FILES = $(wildcard src/*.swift) $(wildcard src/Device/*.swift) $(wildcard *.m)
kdeconnectjb_SWIFT_BRIDGING_HEADER = kdeconnectjb-Bridging-Header.h
kdeconnectjb_PRIVATE_FRAMEWORKS = AppSupport

include $(THEOS_MAKE_PATH)/application.mk

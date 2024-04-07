TARGET := iphone:clang:latest:15.0
INSTALL_TARGET_PROCESSES = SpringBoard

ARCHS = arm64

include $(THEOS)/makefiles/common.mk

TWEAK_NAME = kdeconnectjbtweak

kdeconnectjbtweak_FILES = Tweak.xm
kdeconnectjbtweak_CFLAGS = -fobjc-arc
kdeconnectjbtweak_PRIVATE_FRAMEWORKS = AppSupport SpringBoardUI

include $(THEOS_MAKE_PATH)/tweak.mk

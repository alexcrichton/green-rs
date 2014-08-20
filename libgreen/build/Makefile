ARCH := $(word 1, $(subst -, ,$(TARGET)))

ifneq ($(findstring i686,$(TARGET)),)
CFLAGS += -m32
else
CFLAGS += -m64
endif

all:
	$(CC) $(CFLAGS) -c -o $(OUT_DIR)/context_switch.o $(ARCH)/_context.S
	$(AR) crus $(OUT_DIR)/libcontext_switch.a $(OUT_DIR)/context_switch.o

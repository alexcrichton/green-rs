ARCH := $(word 1, $(subst -, ,$(TARGET)))

ifeq (androideabi,$(findstring androideabi,$(TARGET)))
  CC := $(TARGET)-gcc
  AR := $(TARGET)-ar
else ifneq ($(findstring i686,$(TARGET)),)
  CFLAGS += -m32 -fPIC
else
  CFLAGS += -m64 -fPIC
endif

export PYTHONPATH := $(PYTHONPATH):$(CURDIR)/gyp/pylib

ifneq ($(findstring mingw,$(TARGET)),)
  LIBUV_OSTYPE := win
else ifneq ($(findstring apple-darwin,$(TARGET)),)
  LIBUV_OSTYPE := mac
else ifneq ($(findstring apple-ios,$(TARGET)),)
  LIBUV_OSTYPE := ios
else ifneq ($(findstring unknown-freebsd,$(TARGET)),)
  LIBUV_OSTYPE := freebsd
else ifneq ($(findstring unknown-dragonfly,$(TARGET)),)
  LIBUV_OSTYPE := freebsd
  # required on DragonFly, otherwise gyp fails with a Python exception
  LIBUV_GYP_ARGS := --no-parallel
else ifneq ($(findstring linux-androideabi,$(TARGET)),)
  LIBUV_OSTYPE := android
  LIBUV_ARGS := PLATFORM=android host=android OS=linux
else
  LIBUV_OSTYPE := linux
endif

all: $(OUT_DIR)/libuv.a
	$(CC) $(CFLAGS) -c -o $(OUT_DIR)/rustuv.o rust_uv.c -I libuv/include
	$(AR) crus $(OUT_DIR)/libuv_support.a $(OUT_DIR)/rustuv.o

LIBUV_NO_LOAD = run-benchmarks.target.mk run-tests.target.mk \
		uv_dtrace_header.target.mk uv_dtrace_provider.target.mk

LIBUV_NAME := libuv.a
LIBUV_DIR := $(OUT_DIR)/libuv
LIBUV_LIB := $(OUT_DIR)/$(LIBUV_NAME)
LIBUV_ARCH = $(subst i386,ia32,$(subst x86_64,x64,$(ARCH)))

LIBUV_MAKEFILE := $(OUT_DIR)/libuv/Makefile
LIBUV_BUILD_DIR := $(OUT_DIR)/libuv
LIBUV_XCODEPROJ := $(OUT_DIR)/uv.xcodeproj

$(LIBUV_MAKEFILE):
	(cd libuv && python ./gyp_uv.py -f make -Dtarget_arch=$(LIBUV_ARCH) \
	   -D ninja \
	   -DOS=$(LIBUV_OSTYPE) \
	   -Goutput_dir=$(@D) $(LIBUV_GYP_ARGS) --generator-output $(@D))
	touch $@

# Windows has a completely different build system for libuv because of mingw. In
# theory when we support msvc then we should be using gyp's msvc output instead
# of mingw's makefile for windows
ifneq ($(findstring mingw,$(TARGET)),)
LIBUV_LOCAL := $$(S)src/libuv/libuv.a
$$(LIBUV_LOCAL): $$(LIBUV_DEPS) $$(MKFILE_DEPS)
	$$(Q)$$(MAKE) -C $$(S)src/libuv -f Makefile.mingw \
		LDFLAGS="$$(CFG_GCCISH_LINK_FLAGS)" \
		CC="$$(CC) $$(CFLAGS) $$(SNAP_DEFINES)" \
		CXX="$$(CXX)" \
		AR="$$(AR)" \
		V=$$(VERBOSE)
else ifneq ($(findstring apple-ios,$(TARGET)),)
$(LIBUV_XCODEPROJ):
	cp -rf libuv/ $(LIBUV_BUILD_DIR)
	(cd $(LIBUV_BUILD_DIR) && \
	 python ./gyp_uv.py -f xcode \
	   -D ninja \
	   -R libuv)
	touch $@

LIBUV_XCODE_OUT_LIB := $(LIBUV_BUILD_DIR)/build/Release-$(CFG_SDK_NAME)/libuv.a

$(LIBUV_LIB): $(LIBUV_XCODE_OUT_LIB)
	cp $< $@
$(LIBUV_XCODE_OUT_LIB): $(LIBUV_XCODEPROJ)
	xcodebuild -project $(LIBUV_BUILD_DIR)/uv.xcodeproj \
		CFLAGS="$(CFLAGS) $(SNAP_DEFINES)" \
		$(LIBUV_ARGS) \
		V=$(VERBOSE) \
		-configuration Release \
		-sdk "$(CFG_SDK_NAME)" \
		ARCHS="$(CFG_SDK_ARCHS)"
	touch $@
else
LIBUV_LOCAL := $(LIBUV_DIR)/Release/libuv.a
$(LIBUV_LOCAL): $(LIBUV_MAKEFILE)
	$(MAKE) -C $(LIBUV_BUILD_DIR) \
		CFLAGS="$(CFLAGS)" \
		CC="$(CC)" \
		CXX="$(CXX)" \
		AR="$(AR)" \
		$(LIBUV_ARGS) \
		BUILDTYPE=Release \
		NO_LOAD="$(LIBUV_NO_LOAD)" \
		V=$(VERBOSE)
	touch $@
endif

$(LIBUV_LIB): $(LIBUV_LOCAL)
	cp $< $@

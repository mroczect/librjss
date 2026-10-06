CARGO    ?= cargo
TARGET   ?= release
PROFILE  := $(if $(filter release,$(TARGET)),--release,)
TARGETDIR := $(shell $(CARGO) metadata --no-deps --format-version 1 \
              | python3 -c 'import sys,json;print(json.load(sys.stdin)["target_directory"])')

.PHONY: all clean run

all:
	$(CARGO) build -p librjss-ffi $(PROFILE)

# Build + compile example against the static lib
run: all
	cc -std=c11 -Wall -Wextra -O2 \
	   -I include \
	   examples/hello.c \
	   $(TARGETDIR)/$(if $(filter release,$(TARGET)),release,debug)/liblibrjss_ffi.a \
	   -lpthread -ldl -lm \
	   -o hello
	./hello

clean:
	$(CARGO) clean -p librjss-ffi
	rm -f hello

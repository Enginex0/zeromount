obj-m := nuke.o

ccflags-y += -g0 -Os
ccflags-y += -Wno-declaration-after-statement

KDIR ?= /lib/modules/$(shell uname -r)/build

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

// SPDX-License-Identifier: GPL-2.0-or-later
#include <linux/module.h>
#include <linux/init.h>
#include <linux/fs.h>
#include <linux/path.h>
#include <linux/namei.h>
#include <linux/string.h>
#include <linux/version.h>
#include <linux/kallsyms.h>

#ifndef MODULE
#error "LKM builds only"
#endif

static unsigned long symaddr;
module_param(symaddr, ulong, 0000);

static char *mount_point = "";
module_param(mount_point, charp, 0000);

static void __exit nuke_exit(void) {}

static int call_unregister(struct super_block *sb)
{
    void (*fn)(struct super_block *);
    char buf[KSYM_SYMBOL_LEN] = {0};
    const char *sym = "ext4_unregister_sysfs";

    if (!symaddr) {
        pr_info("zeromount/nuke: symaddr not provided\n");
        return -EINVAL;
    }

    sprint_symbol(buf, symaddr);
    buf[KSYM_SYMBOL_LEN - 1] = '\0';

    if (strncmp(buf, sym, strlen(sym)) != 0) {
        pr_info("zeromount/nuke: wrong symbol: %s\n", buf);
        return -EAGAIN;
    }

    fn = (void (*)(struct super_block *))symaddr;
    fn(sb);
    return 0;
}

static int __init nuke_entry(void)
{
    struct path path;
    struct super_block *sb;
    int err;
    char check[64] = {0};

    if (!mount_point[0]) {
        pr_info("zeromount/nuke: mount_point not provided\n");
        return -EAGAIN;
    }

    err = kern_path(mount_point, 0, &path);
    if (err) {
        pr_info("zeromount/nuke: kern_path failed: %d\n", err);
        return -EAGAIN;
    }

    sb = path.dentry->d_inode->i_sb;
    if (strcmp(sb->s_type->name, "ext4") != 0) {
        pr_info("zeromount/nuke: not ext4 (%s)\n", sb->s_type->name);
        path_put(&path);
        return -EAGAIN;
    }

    snprintf(check, sizeof(check), "/proc/fs/ext4/%s", sb->s_id);
    call_unregister(sb);
    path_put(&path);

    // Verify removal
    err = kern_path(check, 0, &path);
    if (!err) {
        pr_info("zeromount/nuke: %s still exists\n", check);
        path_put(&path);
    } else {
        pr_info("zeromount/nuke: %s removed\n", check);
    }

    return -EAGAIN; // auto-unload
}

module_init(nuke_entry);
module_exit(nuke_exit);
MODULE_LICENSE("GPL");
MODULE_AUTHOR("Enginex0");
MODULE_DESCRIPTION("One-shot ext4 sysfs evidence removal");

#if LINUX_VERSION_CODE >= KERNEL_VERSION(5, 0, 0)
MODULE_IMPORT_NS(VFS_internal_I_am_really_a_filesystem_and_am_NOT_a_driver);
#endif

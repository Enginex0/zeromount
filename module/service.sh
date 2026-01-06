if [ -f /data/adb/nomount/.exclusion_list ]; then
    while IFS= read -r uid; do
        [ -z "$uid" ] && continue
        nm block "$uid"
    done < /data/adb/nomount/.exclusion_list
fi

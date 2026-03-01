-optimizations !code/simplification/cast,!field/*,!class/merging/*
-optimizationpasses 5
-allowaccessmodification

-dontusemixedcaseclassnames
-verbose

-keepattributes *Annotation*

-keepclassmembers enum * {
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

-renamesourcefileattribute SourceFile
-keepattributes SourceFile,LineNumberTable
-repackageclasses ''

-dontobfuscate

package org.telcoray.digitaldetox;
final class NativeSync {
 static { System.loadLibrary("digital_detox_android"); }
 static native String call(String request);
}

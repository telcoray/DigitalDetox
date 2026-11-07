package org.telcoray.digitaldetox;
import android.content.*;
/** Accessibility reconnects after boot. Prompt the user to reopen if background starts are restricted. */
public final class BootReceiver extends BroadcastReceiver {
 @Override public void onReceive(Context c,Intent i){if(!Intent.ACTION_BOOT_COMPLETED.equals(i.getAction()))return;
  var p=c.getSharedPreferences("session",0);if(p.getBoolean("sync",false)||p.getLong("wallEnd",0)>System.currentTimeMillis())try{c.startForegroundService(new Intent(c,DetoxService.class));}catch(RuntimeException ignored){}
 }
}

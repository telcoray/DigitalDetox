package org.telcoray.digitaldetox;
import android.accessibilityservice.AccessibilityService;
import android.view.*;
import android.view.accessibility.AccessibilityEvent;
import android.widget.*;
import android.graphics.Color;
import android.content.*;
import android.os.SystemClock;
import android.telecom.TelecomManager;

public final class BlockService extends AccessibilityService {
 static BlockService instance; View overlay;TextView timer;Button exit;long confirmAt;String foreground="";
 @Override protected void onServiceConnected(){instance=this;refresh();}
 @Override public void onAccessibilityEvent(AccessibilityEvent event){if(event.getPackageName()!=null)foreground=event.getPackageName().toString();refresh();}
 boolean phone(){TelecomManager t=getSystemService(TelecomManager.class);String dialer=t==null?null:t.getDefaultDialerPackage();return foreground.equals(dialer)||foreground.equals("com.android.phone")||foreground.equals("com.android.server.telecom")||foreground.equals("com.android.emergency");}
 void refresh(){DetoxService s=DetoxService.instance;if(s==null||!s.locked()||phone()){remove();return;}
  if(overlay==null){LinearLayout box=new LinearLayout(this);box.setOrientation(LinearLayout.VERTICAL);box.setGravity(Gravity.CENTER);box.setPadding(32,64,32,64);box.setBackgroundColor(Color.rgb(26,26,46));
   TextView title=new TextView(this);title.setText("Time to disconnect");title.setTextColor(Color.WHITE);title.setTextSize(28);box.addView(title);
   timer=new TextView(this);timer.setTextColor(Color.WHITE);timer.setTextSize(60);box.addView(timer);
   TextView hint=new TextView(this);hint.setText("Breathe. Relax your shoulders.\nYour time is yours.");hint.setGravity(Gravity.CENTER);hint.setTextColor(Color.LTGRAY);hint.setTextSize(18);box.addView(hint);
   Button call=new Button(this);call.setText("Phone / emergency calls");call.setOnClickListener(v->{remove();Intent i=new Intent(Intent.ACTION_DIAL).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);try{startActivity(i);}catch(Exception e){refresh();}});box.addView(call);
   exit=new Button(this);exit.setText("Emergency exit");exit.setOnClickListener(v->{if(confirmAt==0){confirmAt=SystemClock.elapsedRealtime()+10000;exit.setText("Wait 10 seconds, then tap to confirm");}else if(SystemClock.elapsedRealtime()>=confirmAt){s.emergency();confirmAt=0;}});box.addView(exit);
   WindowManager.LayoutParams lp=new WindowManager.LayoutParams(-1,-1,WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,android.graphics.PixelFormat.OPAQUE);lp.gravity=Gravity.TOP;try{getSystemService(WindowManager.class).addView(box,lp);overlay=box;}catch(Exception e){s.status="Cannot show block screen: "+e.getMessage();}
  }if(timer!=null)timer.setText(Session.time(s.remaining()));if(confirmAt>0&&SystemClock.elapsedRealtime()>=confirmAt&&exit!=null)exit.setText("Confirm emergency exit (this phone only)");
 }
 void remove(){if(overlay!=null){try{getSystemService(WindowManager.class).removeView(overlay);}catch(Exception ignored){}overlay=null;timer=null;exit=null;confirmAt=0;}}
 @Override public void onInterrupt(){remove();}
 @Override public void onDestroy(){instance=null;remove();super.onDestroy();}
}

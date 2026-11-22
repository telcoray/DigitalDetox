package org.telcoray.digitaldetox;
import android.accessibilityservice.AccessibilityService;
import android.view.*;
import android.view.accessibility.AccessibilityEvent;
import android.widget.*;
import android.graphics.*;
import android.graphics.drawable.GradientDrawable;
import android.content.*;
import android.os.SystemClock;
import android.telecom.TelecomManager;

/**
 * The blocking screen, drawn over every app while this phone is locked. It looks like the
 * Windows lock screen: a breathing guide, the countdown, when it unlocks and a calm message.
 * Phone calls and a delayed emergency exit stay reachable.
 */
public final class BlockService extends AccessibilityService {
 static BlockService instance;
 View overlay;TextView title,timer,unlocks;Button exit;long confirmAt;String foreground="";

 private static final int BG=0xff1a1a2e,EDGE=0xff34495e,WHITE=0xffecf0f1,MUTED=0xffadbcc4,BLUE=0xff3498db;
 // The same messages as on Windows (src/session.rs), one picked for each lock
 private static final String[] MESSAGES={"Take a deep breath. Relax your shoulders.","Your mind deserves this break.",
  "Disconnect to reconnect with yourself.","Breathe in. Breathe out. Just be.","No notifications. No pressure. Just peace.",
  "This time is yours alone.","Let your eyes rest. Let your mind wander.","You're doing great. Keep going.",
  "The world can wait. You matter.","Find calm in the stillness."};

 @Override protected void onServiceConnected(){instance=this;refresh();}
 @Override public void onAccessibilityEvent(AccessibilityEvent event){if(event.getPackageName()!=null)foreground=event.getPackageName().toString();refresh();}
 boolean phone(){TelecomManager t=getSystemService(TelecomManager.class);String dialer=t==null?null:t.getDefaultDialerPackage();return foreground.equals(dialer)||foreground.equals("com.android.phone")||foreground.equals("com.android.server.telecom")||foreground.equals("com.android.emergency");}

 void refresh(){
  DetoxService s=DetoxService.instance;
  if(s==null||!s.locked()||phone()){remove();return;}
  if(overlay==null)show(s);
  boolean pomodoro="pomodoro".equals(s.shared.optString("kind"));
  if(title!=null)title.setText(pomodoro?"Pomodoro break":"Time to disconnect");
  if(timer!=null)timer.setText(Session.time(s.remaining()));
  if(unlocks!=null)unlocks.setText("Unlocks at "+android.text.format.DateFormat.getTimeFormat(this).format(new java.util.Date(System.currentTimeMillis()+s.remaining())));
  if(confirmAt>0&&SystemClock.elapsedRealtime()>=confirmAt&&exit!=null)exit.setText("Confirm emergency exit (this phone only)");
 }

 private void show(DetoxService s){
  FrameLayout root=new FrameLayout(this);root.setBackgroundColor(BG);
  LinearLayout middle=new LinearLayout(this);middle.setOrientation(LinearLayout.VERTICAL);middle.setGravity(Gravity.CENTER_HORIZONTAL);
  middle.setPadding(dp(28),0,dp(28),0);
  title=text("",20,MUTED);middle.addView(title);
  middle.addView(new Breathing(this),new LinearLayout.LayoutParams(dp(200),dp(200)){{topMargin=dp(28);}});
  timer=text("",72,WHITE);timer.setTypeface(Typeface.create("sans-serif-light",Typeface.NORMAL));
  timer.setFontFeatureSettings("tnum");timer.setLetterSpacing(0.02f);
  middle.addView(timer,new LinearLayout.LayoutParams(-2,-2){{topMargin=dp(24);}});
  unlocks=text("",16,MUTED);middle.addView(unlocks);
  TextView message=text(MESSAGES[new java.util.Random().nextInt(MESSAGES.length)],17,WHITE);message.setAlpha(0.85f);
  middle.addView(message,new LinearLayout.LayoutParams(-2,-2){{topMargin=dp(28);}});
  root.addView(middle,new FrameLayout.LayoutParams(-1,-2,Gravity.CENTER));

  LinearLayout bottom=new LinearLayout(this);bottom.setOrientation(LinearLayout.VERTICAL);bottom.setPadding(dp(28),0,dp(28),dp(40));
  Button call=button("Phone / emergency calls");
  call.setOnClickListener(v->{remove();Intent i=new Intent(Intent.ACTION_DIAL).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);try{startActivity(i);}catch(Exception e){refresh();}});
  bottom.addView(call,new LinearLayout.LayoutParams(-1,dp(48)));
  exit=button("Emergency exit");
  exit.setOnClickListener(v->{if(confirmAt==0){confirmAt=SystemClock.elapsedRealtime()+10000;exit.setText("Wait 10 seconds, then tap to confirm");}else if(SystemClock.elapsedRealtime()>=confirmAt){s.emergency();confirmAt=0;}});
  bottom.addView(exit,new LinearLayout.LayoutParams(-1,dp(48)){{topMargin=dp(10);}});
  root.addView(bottom,new FrameLayout.LayoutParams(-1,-2,Gravity.BOTTOM));

  WindowManager.LayoutParams lp=new WindowManager.LayoutParams(-1,-1,WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,PixelFormat.OPAQUE);
  lp.gravity=Gravity.TOP;
  try{getSystemService(WindowManager.class).addView(root,lp);overlay=root;}catch(Exception e){s.status="Cannot show block screen: "+e.getMessage();}
 }

 private TextView text(String value,int size,int color){
  TextView v=new TextView(this);v.setText(value);v.setTextSize(size);v.setTextColor(color);v.setGravity(Gravity.CENTER);return v;
 }
 /** Quiet, outlined: there if needed, not inviting */
 private Button button(String value){
  Button b=new Button(this);b.setText(value);b.setAllCaps(false);b.setTextSize(15);b.setTextColor(MUTED);
  b.setStateListAnimator(null);b.setPadding(dp(8),0,dp(8),0);
  GradientDrawable shape=new GradientDrawable();shape.setColor(BG);shape.setCornerRadius(dp(13));shape.setStroke(dp(1),EDGE);
  b.setBackground(new android.graphics.drawable.RippleDrawable(android.content.res.ColorStateList.valueOf(0x33ffffff),shape,null));
  return b;
 }
 private int dp(int v){return Math.round(v*getResources().getDisplayMetrics().density);}

 /**
  * The breathing guide, as on Windows: a circle grows between two fixed rings in 2 s, shrinks
  * in 3 s and rests for 4 s (Buteyko's reduced breathing), at a steady speed.
  */
 static final class Breathing extends View {
  private final Paint ring=new Paint(Paint.ANTI_ALIAS_FLAG),fill=new Paint(Paint.ANTI_ALIAS_FLAG);
  Breathing(Context c){super(c);
   ring.setStyle(Paint.Style.STROKE);ring.setStrokeWidth(2*c.getResources().getDisplayMetrics().density);ring.setColor(EDGE);
   fill.setColor(BLUE);fill.setAlpha(110);}
  @Override protected void onDraw(Canvas canvas){
   float cx=getWidth()/2f,cy=getHeight()/2f,max=Math.min(cx,cy)-ring.getStrokeWidth(),min=max*0.45f;
   long t=SystemClock.uptimeMillis()%9000;
   float k=t<2000?t/2000f:t<5000?1-(t-2000)/3000f:0;
   canvas.drawCircle(cx,cy,min+(max-min)*k,fill);
   canvas.drawCircle(cx,cy,max,ring);
   canvas.drawCircle(cx,cy,min,ring);
   postInvalidateOnAnimation();
  }
 }

 void remove(){if(overlay!=null){try{getSystemService(WindowManager.class).removeView(overlay);}catch(Exception ignored){}overlay=null;title=null;timer=null;unlocks=null;exit=null;confirmAt=0;}}
 @Override public void onInterrupt(){remove();}
 @Override public void onDestroy(){instance=null;remove();super.onDestroy();}
}

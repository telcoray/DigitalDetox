package org.telcoray.digitaldetox;
import android.app.*;
import android.content.*;
import android.os.*;
import android.net.wifi.WifiManager;
import org.json.*;
import java.math.BigInteger;
import java.net.*;
import java.security.SecureRandom;
import java.util.*;

public final class DetoxService extends Service {
 static DetoxService instance;
 final Handler handler=new Handler(Looper.getMainLooper());
 JSONObject shared=new JSONObject(), pending;
 long version; String author="0",device,status="",code="",escaped="";
 boolean syncEnabled, nativeReady; long lockUntil,wallEnd; String lockKey="";
 // True for the Pomodoro started by itself each day: it stays on this phone, paired devices aren't told
 boolean local;
 // While the daily Pomodoro's notification counts down (elapsedRealtime), else 0
 long offerUntil;
 static final String START_DAILY="start_daily", NOT_TODAY="not_today", START_SET_TIME="start_set_time", SKIP_SET_TIME="skip_set_time";
 // The set time (Schedule tab) whose warning is showing, else null
 Schedule.Due setTime;
 final Map<String,String> peers=new LinkedHashMap<>();final Map<String,Long> seen=new HashMap<>();
 WifiManager.MulticastLock multicast;
 static JSONObject obj(Object... values){JSONObject o=new JSONObject();try{for(int i=0;i<values.length;i+=2)o.put((String)values[i],values[i+1]);}catch(JSONException e){throw new IllegalArgumentException(e);}return o;}
 JSONObject rpc(JSONObject o){try{JSONObject r=new JSONObject(NativeSync.call(o.toString()));if(r.has("error"))status=r.optString("error");return r;}catch(Exception|LinkageError e){status="Sync unavailable: "+e.getMessage();return obj("error",status);}}
 @Override public void onCreate(){super.onCreate();instance=this;
  getSystemService(NotificationManager.class).createNotificationChannel(new NotificationChannel("session","Detox and Sync",NotificationManager.IMPORTANCE_LOW));
  getSystemService(NotificationManager.class).createNotificationChannel(new NotificationChannel("daily","Daily Pomodoro and set times",NotificationManager.IMPORTANCE_HIGH));
  startForeground(1,notification("Digital Detox is ready"));
  var p=getSharedPreferences("session",0);device=p.getString("device",null);if(device==null){device=new BigInteger(64,new SecureRandom()).toString();p.edit().putString("device",device).commit();}
  syncEnabled=p.getBoolean("sync",false);version=p.getLong("version",0);author=p.getString("author","0");escaped=p.getString("escaped","");local=p.getBoolean("local",false);
  try{shared=new JSONObject(p.getString("shared","{\"kind\":\"idle\"}"));}catch(Exception e){shared=obj("kind","idle");}
  try{if(p.contains("pending"))pending=new JSONObject(p.getString("pending","{}"));}catch(Exception ignored){}
  String savedKey=p.getString("lockKey","");long savedEnd=p.getLong("wallEnd",0);if(savedEnd>Session.now()){lockKey=savedKey;wallEnd=savedEnd;lockUntil=SystemClock.elapsedRealtime()+Math.min(999*60000L,savedEnd-Session.now());}
  initSync();handler.post(tick);
 }
 void initSync(){try{JSONArray a=new JSONArray();a.put("255.255.255.255");for(NetworkInterface n:Collections.list(NetworkInterface.getNetworkInterfaces()))if(n.isUp()&&!n.isLoopback())for(InterfaceAddress i:n.getInterfaceAddresses())if(i.getBroadcast()!=null)a.put(i.getBroadcast().getHostAddress());String g=Vault.load(this);JSONObject r=rpc(obj("op","init","device",device,"name",Build.MANUFACTURER+" "+Build.MODEL,"addresses",a,"group",g==null?JSONObject.NULL:new JSONObject(g),"enabled",syncEnabled));nativeReady=!r.has("error");
   if(nativeReady){sendState();WifiManager w=(WifiManager)getApplicationContext().getSystemService(WIFI_SERVICE);multicast=w.createMulticastLock("digital-detox-sync");multicast.setReferenceCounted(false);if(syncEnabled)multicast.acquire();}
  }catch(Exception e){status="Pairing needs setup again: "+e.getMessage();syncEnabled=false;}}
 Notification notification(String text){PendingIntent pi=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_IMMUTABLE);return new Notification.Builder(this,"session").setSmallIcon(android.R.drawable.ic_lock_idle_alarm).setContentTitle("Digital Detox").setContentText(text).setContentIntent(pi).setOngoing(true).build();}
 final Runnable tick=new Runnable(){public void run(){try{
  if(nativeReady){JSONObject polled=rpc(obj("op","poll"));Session.clockOffset=polled.optLong("clock_offset",0);JSONArray events=polled.optJSONArray("events");if(events!=null)for(int i=0;i<events.length();i++)event(events.getJSONObject(i));}
  if(lockUntil==0&&pending!=null){shared=pending;pending=null;save();}long now=Session.now();
  if(lockUntil>0&&SystemClock.elapsedRealtime()>=lockUntil){escaped=lockKey;lockUntil=0;wallEnd=0;lockKey="";if(pending!=null){shared=pending;pending=null;}save();}
  Session.Phase p=Session.phase(shared,now);
  // A Pomodoro with a set number of rounds is over: back to idle, like the device that started it
  if(lockUntil==0&&p.key().equals("done")){update(obj("kind","idle"));p=Session.phase(shared,now);}
  // The lock itself begins further down; meanwhile nothing else may start
  if(!checkMorning()){checkDaily();checkSchedule();}
  if(lockUntil==0&&p.locked()&&!p.key().equals(escaped)&&p.remaining()>=5000&&BlockService.instance!=null){lockKey=p.key();wallEnd=now+Math.min(p.remaining(),999*60000L);lockUntil=SystemClock.elapsedRealtime()+(wallEnd-now);save();}
  if(BlockService.instance!=null)BlockService.instance.refresh();
  getSystemService(NotificationManager.class).notify(1,notification(locked()?"Detox · "+Session.time(remaining()):shared.optString("kind").equals("pomodoro")?"Pomodoro · "+Session.time(p.remaining()):syncEnabled?"Sync enabled on your local network":"Ready"));
 }catch(Exception e){status=e.getMessage();}handler.postDelayed(this,1000);}};
 boolean locked(){return lockUntil>SystemClock.elapsedRealtime();}long remaining(){return Math.max(0,lockUntil-SystemClock.elapsedRealtime());}
 void event(JSONObject e)throws Exception {String type=e.optString("type");if(type.equals("error")){status=e.optString("message");code="";return;}String peer=e.optString("device");if(!peer.isEmpty()){peers.put(peer,e.optString("name"));seen.put(peer,SystemClock.elapsedRealtime());}
  if(type.equals("paired")){if(!e.isNull("group"))Vault.save(this,e.getJSONObject("group").toString());syncEnabled=true;code="";status="Paired with "+e.optString("name");setSync(true);save();}
  if(type.equals("remote")&&syncEnabled){long t=e.optLong("time");String d=e.optString("author","0");if(t>version||(t==version&&new BigInteger(d).compareTo(new BigInteger(author))>0)){JSONObject s=e.getJSONObject("shared");version=t;author=d;local=false;if(locked())pending=s;else shared=s;send(s);save();}}
 }
 void send(JSONObject s){if(nativeReady)rpc(obj("op","publish","time",version,"device",author,"shared",s));}
 void sendState(){if(!local)send(pending==null?shared:pending);}
 void publish(JSONObject s){if(locked())return;shared=s;pending=null;version=Session.now();author=device;escaped="";local=false;save();sendState();}
 /** Like publish, but on this phone only: the version stays, so paired devices aren't told */
 void publishLocal(JSONObject s){if(locked())return;shared=s;pending=null;escaped="";local=true;save();}
 /** A change to the current session (pause, stop): shared only if the session was */
 void update(JSONObject s){if(local)publishLocal(s);else publish(s);}
 void save(){getSharedPreferences("session",0).edit().putString("shared",shared.toString()).putString("pending",pending==null?null:pending.toString()).putLong("version",version).putString("author",author).putBoolean("sync",syncEnabled).putString("escaped",escaped).putLong("wallEnd",wallEnd).putString("lockKey",lockKey).putBoolean("local",local).commit();}
 void emergency(){escaped=lockKey;lockUntil=0;wallEnd=0;lockKey="";if(pending!=null){shared=pending;pending=null;}save();if(BlockService.instance!=null)BlockService.instance.refresh();}
 void setSync(boolean enabled){if(locked())return;syncEnabled=enabled;rpc(obj("op","enable","enabled",enabled));if(multicast!=null){if(enabled&&!multicast.isHeld())multicast.acquire();if(!enabled&&multicast.isHeld())multicast.release();}save();}
 void join(String c){rpc(obj("op","join","code",c));status="Looking for the device showing the code…";}
 void invite(){try{if(Vault.load(this)==null){byte[] k=new byte[32];new SecureRandom().nextBytes(k);JSONArray bytes=new JSONArray();for(byte b:k)bytes.put(b&255);JSONObject g=obj("id",new BigInteger(64,new SecureRandom()).toString(),"key",bytes);Vault.save(this,g.toString());rpc(obj("op","group","group",g));}code=rpc(obj("op","invite")).optString("code");status="Code valid for two minutes";}catch(Exception e){status=e.getMessage();}}
 void unpair(){if(locked())return;setSync(false);rpc(obj("op","cancel"));rpc(obj("op","group","group",JSONObject.NULL));Vault.clear(this);peers.clear();code="";status="Unpaired";}
 @Override public int onStartCommand(Intent i,int f,int id){String a=i==null?null:i.getAction();if(START_DAILY.equals(a)){if(offerUntil>0)startDaily();}else if(NOT_TODAY.equals(a))cancelOffer();
  else if(START_SET_TIME.equals(a)){if(setTime!=null)startSetTime(setTime);}else if(SKIP_SET_TIME.equals(a)){if(setTime!=null)handled(setTime);}
  return START_STICKY;}

 // The Schedule tab: a notification a minute before a set time, then this phone locks (this
 // phone only, like on Windows). Skipped during a detox or a Pomodoro, which has breaks of its own.
 void checkSchedule(){
  var prefs=getSharedPreferences("schedule",0);
  java.time.LocalDateTime now=java.time.LocalDateTime.now();
  Schedule.Due due=Schedule.due(Schedule.load(this),now,prefs.getString("handled",""));
  if(due==null){if(setTime!=null){setTime=null;getSystemService(NotificationManager.class).cancel(3);}return;}
  if(locked()||running()){handled(due);return;}
  long until=java.time.Duration.between(now,due.at()).toMillis();
  if(until<=0){startSetTime(due);return;}
  setTime=due;
  PendingIntent start=PendingIntent.getService(this,3,new Intent(this,DetoxService.class).setAction(START_SET_TIME),PendingIntent.FLAG_IMMUTABLE);
  PendingIntent skip=PendingIntent.getService(this,4,new Intent(this,DetoxService.class).setAction(SKIP_SET_TIME),PendingIntent.FLAG_IMMUTABLE);
  PendingIntent open=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_IMMUTABLE);
  getSystemService(NotificationManager.class).notify(3,new Notification.Builder(this,"daily").setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
   .setContentTitle("Detox at "+PomodoroSettings.timeText(due.at().getHour()*60+due.at().getMinute()))
   .setContentText("Locks in "+(until+999)/1000+" s, for "+due.minutes()+" minutes.").setContentIntent(open)
   .setOnlyAlertOnce(true).setOngoing(true).addAction(new Notification.Action.Builder(null,"Start now",start).build())
   .addAction(new Notification.Action.Builder(null,"Skip",skip).build()).build());
 }
 /** Done with this set time: started, skipped, or let go */
 void handled(Schedule.Due due){
  getSharedPreferences("schedule",0).edit().putString("handled",due.key()).apply();
  setTime=null;getSystemService(NotificationManager.class).cancel(3);
 }
 void startSetTime(Schedule.Due due){
  handled(due);
  if(locked()||running())return;
  publishLocal(obj("kind","detox","ends_at",Session.now()+due.minutes()*60000L));
 }

 // "Wait in the morning" (see MorningLock): the first time the phone is unlocked each day, it
 // locks right away for the chosen minutes, on this phone only. True when it started now.
 boolean checkMorning(){
  java.time.LocalDateTime now=java.time.LocalDateTime.now();
  if(!MorningLock.due(this,now)||locked())return false;
  PowerManager power=getSystemService(PowerManager.class);KeyguardManager keyguard=getSystemService(KeyguardManager.class);
  if(!power.isInteractive()||keyguard.isKeyguardLocked()||BlockService.instance==null)return false;
  MorningLock.markDone(this,now);
  // A Pomodoro that's going on (e.g. from the PC) has breaks of its own; one left paused
  // since yesterday doesn't count, and paired devices that followed it stop it too
  if(running()){
   if(!shared.has("paused_at")||shared.optLong("paused_at")>=MorningLock.dayStart(now))return false;
   if(!local)publish(obj("kind","idle"));
  }
  cancelOffer();
  publishLocal(obj("kind","detox","ends_at",Session.now()+MorningLock.minutes(this)*60000L));
  return true;
 }

 // "Start on its own each day" (see PomodoroSettings): the first time the phone is unlocked
 // after the chosen time, a notification counts down 30 s, then a Pomodoro starts here only.
 void checkDaily(){
  if(offerUntil>0){tickOffer();return;}
  java.time.LocalDateTime now=java.time.LocalDateTime.now();
  if(!PomodoroSettings.due(this,now)||locked())return;
  // Already running today (started by hand or by a paired device): nothing to offer
  if(running()&&!leftPaused()){PomodoroSettings.markDone(this,now.toLocalDate());return;}
  // Someone using the phone, past the lock screen, and breaks can block
  PowerManager power=getSystemService(PowerManager.class);KeyguardManager keyguard=getSystemService(KeyguardManager.class);
  if(!power.isInteractive()||keyguard.isKeyguardLocked()||BlockService.instance==null)return;
  PomodoroSettings.markDone(this,now.toLocalDate());
  offerUntil=SystemClock.elapsedRealtime()+30000;tickOffer();
 }
 boolean running(){return shared.optString("kind").equals("pomodoro");}
 /** A Pomodoro paused before today's start time and never resumed: today's replaces it */
 boolean leftPaused(){
  if(!running()||!shared.has("paused_at"))return false;
  java.time.ZonedDateTime start=java.time.LocalDate.now().atTime(PomodoroSettings.from(this)/60,PomodoroSettings.from(this)%60).atZone(java.time.ZoneId.systemDefault());
  return shared.optLong("paused_at")<start.toInstant().toEpochMilli();
 }
 void tickOffer(){
  // Started some other way meanwhile, or a detox began
  if(locked()||(running()&&!leftPaused())){cancelOffer();return;}
  long left=offerUntil-SystemClock.elapsedRealtime();
  if(left<=0){startDaily();return;}
  PendingIntent start=PendingIntent.getService(this,1,new Intent(this,DetoxService.class).setAction(START_DAILY),PendingIntent.FLAG_IMMUTABLE);
  PendingIntent skip=PendingIntent.getService(this,2,new Intent(this,DetoxService.class).setAction(NOT_TODAY),PendingIntent.FLAG_IMMUTABLE);
  PendingIntent open=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_IMMUTABLE);
  getSystemService(NotificationManager.class).notify(2,new Notification.Builder(this,"daily").setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
   .setContentTitle("Pomodoro starting").setContentText("Starts in "+(left+999)/1000+" s, on this phone only.").setContentIntent(open)
   .setOnlyAlertOnce(true).setOngoing(true).addAction(new Notification.Action.Builder(null,"Start now",start).build())
   .addAction(new Notification.Action.Builder(null,"Not today",skip).build()).build());
 }
 void cancelOffer(){offerUntil=0;getSystemService(NotificationManager.class).cancel(2);}
 void startDaily(){
  cancelOffer();
  if(locked())return;
  if(running()&&!leftPaused())return;
  // One paused since an earlier day, that paired devices followed: they stop it too
  if(running()&&!local)publish(obj("kind","idle"));
  publishLocal(obj("kind","pomodoro","start",Session.now(),"plan",PomodoroSettings.plan(this)));
 }
 @Override public IBinder onBind(Intent i){return null;}
 @Override public void onDestroy(){handler.removeCallbacks(tick);rpc(obj("op","close"));if(multicast!=null&&multicast.isHeld())multicast.release();instance=null;if(BlockService.instance!=null)BlockService.instance.refresh();super.onDestroy();}
}

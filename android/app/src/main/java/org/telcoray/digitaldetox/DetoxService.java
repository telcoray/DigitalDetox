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
 final Map<String,String> peers=new LinkedHashMap<>();final Map<String,Long> seen=new HashMap<>();
 WifiManager.MulticastLock multicast;
 static JSONObject obj(Object... values){JSONObject o=new JSONObject();try{for(int i=0;i<values.length;i+=2)o.put((String)values[i],values[i+1]);}catch(JSONException e){throw new IllegalArgumentException(e);}return o;}
 JSONObject rpc(JSONObject o){try{JSONObject r=new JSONObject(NativeSync.call(o.toString()));if(r.has("error"))status=r.optString("error");return r;}catch(Exception|LinkageError e){status="Sync unavailable: "+e.getMessage();return obj("error",status);}}
 @Override public void onCreate(){super.onCreate();instance=this;
  getSystemService(NotificationManager.class).createNotificationChannel(new NotificationChannel("session","Detox and Sync",NotificationManager.IMPORTANCE_LOW));
  startForeground(1,notification("Digital Detox is ready"));
  var p=getSharedPreferences("session",0);device=p.getString("device",null);if(device==null){device=new BigInteger(64,new SecureRandom()).toString();p.edit().putString("device",device).commit();}
  syncEnabled=p.getBoolean("sync",false);version=p.getLong("version",0);author=p.getString("author","0");escaped=p.getString("escaped","");
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
  if(lockUntil==0&&p.key().equals("done")){publish(obj("kind","idle"));p=Session.phase(shared,now);}
  if(lockUntil==0&&p.locked()&&!p.key().equals(escaped)&&p.remaining()>=5000&&BlockService.instance!=null){lockKey=p.key();wallEnd=now+Math.min(p.remaining(),999*60000L);lockUntil=SystemClock.elapsedRealtime()+(wallEnd-now);save();}
  if(BlockService.instance!=null)BlockService.instance.refresh();
  getSystemService(NotificationManager.class).notify(1,notification(locked()?"Detox · "+Session.time(remaining()):shared.optString("kind").equals("pomodoro")?"Pomodoro · "+Session.time(p.remaining()):syncEnabled?"Sync enabled on your local network":"Ready"));
 }catch(Exception e){status=e.getMessage();}handler.postDelayed(this,1000);}};
 boolean locked(){return lockUntil>SystemClock.elapsedRealtime();}long remaining(){return Math.max(0,lockUntil-SystemClock.elapsedRealtime());}
 void event(JSONObject e)throws Exception {String type=e.optString("type");if(type.equals("error")){status=e.optString("message");code="";return;}String peer=e.optString("device");if(!peer.isEmpty()){peers.put(peer,e.optString("name"));seen.put(peer,SystemClock.elapsedRealtime());}
  if(type.equals("paired")){if(!e.isNull("group"))Vault.save(this,e.getJSONObject("group").toString());syncEnabled=true;code="";status="Paired with "+e.optString("name");setSync(true);save();}
  if(type.equals("remote")&&syncEnabled){long t=e.optLong("time");String d=e.optString("author","0");if(t>version||(t==version&&new BigInteger(d).compareTo(new BigInteger(author))>0)){JSONObject s=e.getJSONObject("shared");version=t;author=d;if(locked())pending=s;else shared=s;send(s);save();}}
 }
 void send(JSONObject s){if(nativeReady)rpc(obj("op","publish","time",version,"device",author,"shared",s));}
 void sendState(){send(pending==null?shared:pending);}
 void publish(JSONObject s){if(locked())return;shared=s;pending=null;version=Session.now();author=device;escaped="";save();sendState();}
 void save(){getSharedPreferences("session",0).edit().putString("shared",shared.toString()).putString("pending",pending==null?null:pending.toString()).putLong("version",version).putString("author",author).putBoolean("sync",syncEnabled).putString("escaped",escaped).putLong("wallEnd",wallEnd).putString("lockKey",lockKey).commit();}
 void emergency(){escaped=lockKey;lockUntil=0;wallEnd=0;lockKey="";if(pending!=null){shared=pending;pending=null;}save();if(BlockService.instance!=null)BlockService.instance.refresh();}
 void setSync(boolean enabled){if(locked())return;syncEnabled=enabled;rpc(obj("op","enable","enabled",enabled));if(multicast!=null){if(enabled&&!multicast.isHeld())multicast.acquire();if(!enabled&&multicast.isHeld())multicast.release();}save();}
 void join(String c){rpc(obj("op","join","code",c));status="Looking for the device showing the code…";}
 void invite(){try{if(Vault.load(this)==null){byte[] k=new byte[32];new SecureRandom().nextBytes(k);JSONArray bytes=new JSONArray();for(byte b:k)bytes.put(b&255);JSONObject g=obj("id",new BigInteger(64,new SecureRandom()).toString(),"key",bytes);Vault.save(this,g.toString());rpc(obj("op","group","group",g));}code=rpc(obj("op","invite")).optString("code");status="Code valid for two minutes";}catch(Exception e){status=e.getMessage();}}
 void unpair(){if(locked())return;setSync(false);rpc(obj("op","cancel"));rpc(obj("op","group","group",JSONObject.NULL));Vault.clear(this);peers.clear();code="";status="Unpaired";}
 @Override public int onStartCommand(Intent i,int f,int id){return START_STICKY;}
 @Override public IBinder onBind(Intent i){return null;}
 @Override public void onDestroy(){handler.removeCallbacks(tick);rpc(obj("op","close"));if(multicast!=null&&multicast.isHeld())multicast.release();instance=null;if(BlockService.instance!=null)BlockService.instance.refresh();super.onDestroy();}
}

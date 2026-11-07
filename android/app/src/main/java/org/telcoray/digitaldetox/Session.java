package org.telcoray.digitaldetox;
import org.json.JSONObject;
/** Pure scheduling logic, shared wall-clock schedule compatible with Windows. */
final class Session {
 static final long WORK=25*60000L, SHORT=5*60000L, LONG=15*60000L, CYCLE=130*60000L;
 record Phase(boolean locked,long remaining,int round,String key) {}
 static Phase phase(JSONObject s,long now) {
  String kind=s.optString("kind","idle");
  if(kind.equals("detox"))return new Phase(s.optLong("ends_at")>now,Math.max(0,s.optLong("ends_at")-now),0,"detox:"+s.optLong("ends_at"));
  if(!kind.equals("pomodoro"))return new Phase(false,0,0,"idle");
  long start=s.optLong("start"), effective=s.has("paused_at")?s.optLong("paused_at"):now;
  long elapsed=Math.max(0,effective-start), cycle=elapsed/CYCLE,t=elapsed%CYCLE;
  for(int round=1;round<=4;round++){if(t<WORK)return new Phase(false,WORK-t,round,"work:"+start+":"+cycle+":"+round);t-=WORK;long b=round==4?LONG:SHORT;if(t<b)return new Phase(!s.has("paused_at"),b-t,round,"break:"+start+":"+cycle+":"+round);t-=b;}
  return new Phase(false,0,0,"idle");
 }
 static String time(long millis){long s=(millis+999)/1000;return String.format(java.util.Locale.ROOT,"%02d:%02d",s/60,s%60);}
}

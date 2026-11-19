package org.telcoray.digitaldetox;
import org.json.JSONObject;
/** Pure scheduling logic, shared wall-clock schedule compatible with Windows (src/pomodoro.rs). */
final class Session {
 static final long WORK=25*60000L, SHORT=5*60000L, LONG=15*60000L, CYCLE=130*60000L;
 record Phase(boolean locked,long remaining,int round,String key) {}
 /** The lengths chosen on the device that started the Pomodoro; the usual 25/5/15 × 4 if it didn't say. */
 record Plan(long work,long shortBreak,long longBreak,int rounds,int total) {
  static Plan of(JSONObject shared){
   JSONObject p=shared.optJSONObject("plan");if(p==null)p=new JSONObject();
   return new Plan(minutes(p,"work",25,180),minutes(p,"short_break",5,120),minutes(p,"long_break",15,120),
    clamp(p.optInt("rounds",4),1,8),clamp(p.optInt("total",0),0,50));
  }
  static long minutes(JSONObject p,String name,int usual,int max){return clamp(p.optInt(name,usual),1,max)*60000L;}
  static int clamp(int v,int min,int max){return Math.max(min,Math.min(max,v));}
  long breakAfter(int round){return round==rounds?longBreak:shortBreak;}
  long cycle(){long c=0;for(int r=1;r<=rounds;r++)c+=work+breakAfter(r);return c;}
 }
 static Phase phase(JSONObject s,long now) {
  String kind=s.optString("kind","idle");
  if(kind.equals("detox"))return new Phase(s.optLong("ends_at")>now,Math.max(0,s.optLong("ends_at")-now),0,"detox:"+s.optLong("ends_at"));
  if(!kind.equals("pomodoro"))return new Phase(false,0,0,"idle");
  Plan plan=Plan.of(s);
  long start=s.optLong("start"), effective=s.has("paused_at")?s.optLong("paused_at"):now;
  long elapsed=Math.max(0,effective-start),cycle=0,t=elapsed;int count=plan.total();
  if(plan.total()==0){cycle=elapsed/plan.cycle();t=elapsed%plan.cycle();count=plan.rounds();}
  for(int n=1;n<=count;n++){int round=(n-1)%plan.rounds()+1;
   if(t<plan.work())return new Phase(false,plan.work()-t,round,"work:"+start+":"+cycle+":"+n);t-=plan.work();
   long b=plan.breakAfter(round);if(t<b)return new Phase(!s.has("paused_at"),b-t,round,"break:"+start+":"+cycle+":"+n);t-=b;}
  return new Phase(false,0,0,plan.total()==0?"idle":"done");
 }
 static String time(long millis){long s=(millis+999)/1000;return String.format(java.util.Locale.ROOT,"%02d:%02d",s/60,s%60);}
}

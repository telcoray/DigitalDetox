package org.telcoray.digitaldetox;
import org.junit.Test;
import org.json.JSONObject;
import static org.junit.Assert.*;
public class SessionTest {
 @Test public void detoxExpires() throws Exception {JSONObject s=new JSONObject("{\"kind\":\"detox\",\"ends_at\":60000}");assertTrue(Session.phase(s,59999).locked());assertFalse(Session.phase(s,60000).locked());}
 @Test public void scheduleMatchesWindows() throws Exception {JSONObject s=new JSONObject("{\"kind\":\"pomodoro\",\"start\":1000}");assertFalse(Session.phase(s,1000).locked());assertTrue(Session.phase(s,1000+Session.WORK).locked());assertEquals(Session.SHORT,Session.phase(s,1000+Session.WORK).remaining());assertEquals(Session.LONG,Session.phase(s,1000+4*Session.WORK+3*Session.SHORT).remaining());assertEquals(1,Session.phase(s,1000+Session.CYCLE).round());assertFalse(Session.phase(s,1000+Session.CYCLE).locked());}
 @Test public void pauseFreezesWork() throws Exception {JSONObject s=new JSONObject("{\"kind\":\"pomodoro\",\"start\":0,\"paused_at\":60000}");assertEquals(Session.WORK-60000,Session.phase(s,99999999).remaining());assertFalse(Session.phase(s,99999999).locked());}
 @Test public void laterCycleHasDifferentEmergencyKey() throws Exception {JSONObject s=new JSONObject("{\"kind\":\"pomodoro\",\"start\":0}");assertNotEquals(Session.phase(s,Session.WORK).key(),Session.phase(s,Session.CYCLE+Session.WORK).key());}
}

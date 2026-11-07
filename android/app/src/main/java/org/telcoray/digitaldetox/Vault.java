package org.telcoray.digitaldetox;
import android.content.Context;
import android.security.keystore.*;
import android.util.Base64;
import java.security.KeyStore;
import javax.crypto.*;
import javax.crypto.spec.GCMParameterSpec;
final class Vault {
 static SecretKey key() throws Exception {
  KeyStore ks=KeyStore.getInstance("AndroidKeyStore");ks.load(null);
  if(!ks.containsAlias("detox-sync")) { KeyGenerator g=KeyGenerator.getInstance("AES","AndroidKeyStore");g.init(new KeyGenParameterSpec.Builder("detox-sync",KeyProperties.PURPOSE_ENCRYPT|KeyProperties.PURPOSE_DECRYPT).setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build());g.generateKey(); }
  return (SecretKey)ks.getKey("detox-sync",null);
 }
 static void save(Context c,String group) throws Exception { Cipher x=Cipher.getInstance("AES/GCM/NoPadding");x.init(Cipher.ENCRYPT_MODE,key());c.getSharedPreferences("vault",0).edit().putString("iv",Base64.encodeToString(x.getIV(),0)).putString("data",Base64.encodeToString(x.doFinal(group.getBytes(java.nio.charset.StandardCharsets.UTF_8)),0)).commit(); }
 static String load(Context c) throws Exception { var p=c.getSharedPreferences("vault",0);String s=p.getString("data",null);if(s==null)return null;Cipher x=Cipher.getInstance("AES/GCM/NoPadding");x.init(Cipher.DECRYPT_MODE,key(),new GCMParameterSpec(128,Base64.decode(p.getString("iv",""),0)));return new String(x.doFinal(Base64.decode(s,0)),java.nio.charset.StandardCharsets.UTF_8); }
 static void clear(Context c){c.getSharedPreferences("vault",0).edit().clear().commit();}
}

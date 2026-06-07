import com.example.app.Handler;
import com.example.util.Logger;

interface com.example.app.IDriveable
{
   function drive(target:Number):Void;
   function get speed():Number;
}

/**
 * Account handler — AS2 (AVM1) style as emitted by FFDec:
 * top-level dotted class name, no package wrapper.
 */
class com.example.app.Account extends com.example.app.Handler implements com.example.app.IDriveable
{
   private var _sName:String;
   public var ID:Number;
   static var DEFAULT_SPEED:Number = 3;

   function Account(name:String)
   {
      super();
      this._sName = name;
      Logger.dbg("Account created");
   }

   public function logon(user:String, pass:String):Boolean
   {
      var ok:Boolean = this.checkCredentials(user, pass);
      if(ok)
      {
         this.api.kernel.connect();
      }
      return ok;
   }

   function checkCredentials(u:String, p:String):Boolean
   {
      return u.length > 0 && p.length > 0;
   }

   function drive(target:Number):Void
   {
      this.ID = target;
   }

   function get speed():Number
   {
      return Account.DEFAULT_SPEED;
   }
}

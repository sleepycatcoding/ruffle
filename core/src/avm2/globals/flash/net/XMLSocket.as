package flash.net {
    import flash.events.EventDispatcher;

    public class XMLSocket extends EventDispatcher {
        public function XMLSocket(host: String = null, port: int = 0) {
            this.timeout = 20000;
            if (host != null) {
                this.connect(host, port);
            }
        }

        public native function get timeout():int;
        public native function set timeout(value:int):void;

        public native function get connected():Boolean;

        public native function close():void;

        public native function connect(host: String, port: int):void;

        public native function send(object: *): void;
    }
}

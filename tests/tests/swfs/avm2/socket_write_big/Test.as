package
{
    import flash.display.Sprite;

    public class Test extends Sprite
    {
    }
}

import flash.net.Socket;
import flash.net.ObjectEncoding;
import flash.events.Event;
import flash.utils.ByteArray;
import flash.utils.Endian;

var socket:Socket = new Socket();

socket.endian = Endian.BIG_ENDIAN;

socket.addEventListener(Event.CONNECT, function(evt:Event):void
{
    trace("connected");

    trace("writeBoolean()");
    socket.writeBoolean(true);

    trace("writeByte()");
    socket.writeByte(67);
    socket.writeByte(255);

    trace("writeBytes()");
    var byteArray:ByteArray = new ByteArray();
    byteArray.writeByte(10);
    byteArray.writeByte(20);
    byteArray.writeByte(65);
    socket.writeBytes(byteArray, 0, 0);

    trace("writeDouble()");
    socket.writeDouble(8090.76);

    trace("writeFloat()");
    socket.writeFloat(76.6555);

    trace("writeInt()");
    socket.writeInt(-2376553554);

    trace("writeMultiByte()");
    socket.writeMultiByte("Hello!", "utf-8");

    trace("writeObject() (AMF3)");
    socket.objectEncoding = ObjectEncoding.AMF3;
    socket.writeObject(new Object());

    trace("writeObject() (AMF0)");
    socket.objectEncoding = ObjectEncoding.AMF0;
    socket.writeObject(new Object());

    trace("writeShort()");
    socket.writeShort(65535);

    trace("writeUnsignedInt()");
    socket.writeUnsignedInt(4000565000);

    trace("writeUTF()");
    socket.writeUTF("Hello from Ruffle Socket!");

    trace("writeUTFBytes()");
    socket.writeUTFBytes("Raw UTF is cool");

    trace("flush()");
    socket.flush();

    trace("close()");
    socket.close();

});

socket.connect("localhost", 8001);

// This is a stub - the actual class is defined in `object.rs`
package {
	// NOTE: An annoying hack, required since the propertyIsEnumerable method includes this namespace in the signature.
	public namespace AS3 = "http://adobe.com/AS3/2006/builtin";

	public dynamic class Object {
		// NOTE: Stub to make the compiler happy.
		AS3 function propertyIsEnumerable(propertyName:*):Boolean {
			return false;
		}
	}
}

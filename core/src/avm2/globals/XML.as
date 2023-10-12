package {
    [Ruffle(InstanceAllocator)]
    [Ruffle(CallHandler)]
    public final dynamic class XML {
        import __ruffle__.stub_method;

        AS3 function normalize(): XML {
            stub_method("XML", "normalize");
            return this;
        }

        AS3 static function setSettings(settings:Object = null): void {
            if (settings == null) {
                settings = XML.AS3::defaultSettings();
            }
            if ("ignoreComments" in settings) {
                XML.ignoreComments = settings.ignoreComments;
            }
            if ("ignoreProcessingInstructions" in settings) {
                XML.ignoreProcessingInstructions = settings.ignoreProcessingInstructions;
            }
            if ("ignoreWhitespace" in settings) {
                XML.ignoreWhitespace = settings.ignoreWhitespace;
            }
            if ("prettyIndent" in settings) {
                XML.prettyIndent = settings.prettyIndent;
            }
            if ("prettyPrinting" in settings) {
                XML.prettyPrinting = settings.prettyPrinting;
            }
        }

        AS3 static function settings():Object {
            return {
                ignoreComments: XML.ignoreComments,
                ignoreProcessingInstructions: XML.ignoreProcessingInstructions,
                ignoreWhitespace: XML.ignoreWhitespace,
                prettyIndent: XML.prettyIndent,
                prettyPrinting: XML.prettyPrinting
            };
        }

        AS3 static function defaultSettings():Object {
            return {
                ignoreComments: true,
                ignoreProcessingInstructions: true,
                ignoreWhitespace: true,
                prettyIndent: 2,
                prettyPrinting: true
            };
        }

        public function XML(value:* = undefined) {
            this.init(value, XML.ignoreComments, XML.ignoreProcessingInstructions, XML.ignoreWhitespace);
        }

        private native function init(value:*, ignoreComments:Boolean, ignoreProcessingInstructions:Boolean, ignoreWhitespace:Boolean):void;

        AS3 native function hasComplexContent():Boolean;
        AS3 native function hasSimpleContent():Boolean;
        AS3 native function name():Object;
        AS3 native function setName(name:*):void;
        private native function namespace_internal_impl(hasPrefix:Boolean, prefix:String = null):*;
        AS3 function namespace(prefix:String = null):* {
            return namespace_internal_impl(arguments.length > 0, prefix);
        }
        AS3 native function localName():Object;
        AS3 native function toXMLString():String;
        AS3 native function child(name:Object):XMLList;
        AS3 native function childIndex():int;
        AS3 native function children():XMLList;
        AS3 native function contains(value:XML):Boolean;
        AS3 native function copy():XML;
        AS3 native function parent():*;
        AS3 native function elements(name:*):XMLList;
        AS3 native function attributes():XMLList;
        AS3 native function attribute(name:*):XMLList;
        AS3 native function nodeKind():String;
        AS3 native function appendChild(child:Object):XML;
        AS3 native function prependChild(child:Object):XML;
        AS3 native function descendants(name:Object = "*"):XMLList;
        AS3 native function text():XMLList;
        AS3 native function toString():String;
        AS3 native function length():int;
        AS3 native function comments():XMLList;
        AS3 native function processingInstructions(name:String = "*"):XMLList;
        AS3 native function insertChildAfter(child1:Object, child2:Object):*;
        AS3 native function insertChildBefore(child1:Object, child2:Object):*;
        // NOTE: Docs lie, value can be anything not just XML.
        AS3 native function replace(propertyName:Object, value:*):XML;

        AS3 function valueOf():XML {
            return this;
        }

        AS3 function toJSON(k:String) : * {
            return this.toJSON(k);
        }

        // ECMA-357 13.4.4.30 XML.prototype.propertyIsEnumerable ( P )
        override AS3 function propertyIsEnumerable(propertyName:*):Boolean {
            // 1. Return the result of the comparison ToString(P) == "0"
            return propertyName == "0";
        }

        public static var ignoreComments:Boolean = true;
        public static var ignoreProcessingInstructions:Boolean = true;
        public static var ignoreWhitespace:Boolean = true;
        public static var prettyPrinting:Boolean = true;
        public static var prettyIndent:int = 2;

        prototype.hasComplexContent = function():Boolean {
            var self:XML = this;
            return self.AS3::hasComplexContent();
        }

        prototype.hasSimpleContent = function():Boolean {
            var self:XML = this;
            return self.AS3::hasSimpleContent();
        }

        prototype.name = function():Object {
            var self:XML = this;
            // NOTE - `self.name()` should be sufficient here (and in all of the other methods)
            // However, asc.jar doesn't resolve the 'AS3' namespace when I do
            // 'self.name()' here, which leads to the prototype method invoking
            // itself, instead of the AS3 method.
            return self.AS3::name();
        };

        prototype.namespace = function(prefix:String = null):* {
            var self:XML = this;
            return self.AS3::namespace.apply(self, arguments);
        }

        prototype.localName = function():Object {
            var self:XML = this;
            return self.AS3::localName();
        };

        prototype.toXMLString = function():String {
            var self:XML = this;
            return self.AS3::toXMLString();
        };

        prototype.child = function(name:Object):XMLList {
            var self:XML = this;
            return self.AS3::child(name);
        };

        prototype.childIndex = function():XMLList {
            var self:XML = this;
            return self.AS3::childIndex();
        };

        prototype.children = function():XMLList {
            var self:XML = this;
            return self.AS3::children();
        };

        prototype.contains = function(value:XML):Boolean {
            var self:XML = this;
            return self.AS3::contains(value);
        }

        prototype.copy = function():XML {
            var self:XML = this;
            return self.AS3::copy();
        }

        prototype.parent = function():* {
            var self:XML = this;
            return self.AS3::parent();
        };

        prototype.elements = function(name:*):XMLList {
            var self:XML = this;
            return self.AS3::elements(name);
        }

        prototype.toString = function():String {
            if (this === prototype) {
                return "";
            }
            var self:XML = this;
            return self.AS3::toString();
        };

        prototype.attributes = function():XMLList {
            var self:XML = this;
            return self.AS3::attributes();
        };

        prototype.attribute = function(name:*):XMLList {
            var self:XML = this;
            return self.AS3::attribute(name);
        };

        prototype.nodeKind = function():String {
            var self:XML = this;
            return self.AS3::nodeKind();
        };

        prototype.appendChild = function(child:Object):XML {
            var self:XML = this;
            return self.AS3::appendChild(child);
        };

        prototype.prependChild = function(child:Object):XML {
            var self:XML = this;
            return self.AS3::prependChild(child);
        };

        prototype.descendants = function(name:Object):XMLList {
            var self:XML = this;
            return self.AS3::descendants(name);
        };

        prototype.text = function():XMLList {
            var self:XML = this;
            return self.AS3::text();
        };

        prototype.normalize = function():XML {
            var self:XML = this;
            return self.AS3::normalize();
        };

        prototype.length = function():int {
            var self:XML = this;
            return self.AS3::length();
        }

        prototype.toJSON = function(k:String):* {
            return "XML";
        };

        prototype.comments = function():XMLList {
            var self:XML = this;
            return self.AS3::comments();
        }

        prototype.processingInstructions = function(name:String = "*"):XMLList {
            var self:XML = this;
            return self.AS3::processingInstructions(name);
        }

        prototype.insertChildAfter = function(child1:Object, child2:Object):* {
            var self:XML = this;
            return self.AS3::insertChildAfter(child1, child2);
        }

        prototype.insertChildBefore = function(child1:Object, child2:Object):* {
            var self:XML = this;
            return self.AS3::insertChildBefore(child1, child2);
        }

        prototype.replace = function(propertyName:Object, value:*):XML {
            var self:XML = this;
            return self.AS3::replace(propertyName, value);
        }

        prototype.propertyIsEnumerable = function(propertyName:*):Boolean {
            var self:XML = this;
            return self.AS3::propertyIsEnumerable(propertyName);
        }

        public static const length:int = 1;
    }
}

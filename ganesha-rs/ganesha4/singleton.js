var Singleton = /** @class */ (function () {
    function Singleton() {
        // Private constructor to prevent direct instantiation
    }
    Singleton.getInstance = function () {
        if (!Singleton.instance) {
            Singleton.instance = new Singleton();
        }
        return Singleton.instance;
    };
    Singleton.prototype.someBusinessLogic = function () {
        console.log('Singleton is doing something!');
    };
    return Singleton;
}());
// Usage
var singleton1 = Singleton.getInstance();
var singleton2 = Singleton.getInstance();
if (singleton1 === singleton2) {
    console.log('Singleton works, both variables contain the same instance.');
}
else {
    console.log('Singleton failed, variables contain different instances.');
}
singleton1.someBusinessLogic();

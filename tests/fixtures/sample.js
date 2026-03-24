/**
 * Sample JavaScript file exercising JS grammar path.
 */

const DEFAULT_TIMEOUT = 5000;

/** Base class for all handlers. */
class Handler {
    constructor(name) {
        this.name = name;
    }

    handle(request) {
        console.log(`${this.name} handling request`);
        return this.process(request);
    }

    process(request) {
        throw new Error("Not implemented");
    }
}

class JsonHandler extends Handler {
    constructor() {
        super("json");
    }

    process(request) {
        return JSON.parse(request.body);
    }
}

async function fetchData(url) {
    const response = await fetch(url);
    return response.json();
}

const double = (x) => x * 2;

export { Handler, JsonHandler, fetchData, double };

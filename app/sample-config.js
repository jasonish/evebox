/**
 * Sample configuration file - THIS FILE SUBJECT TO CHANGE.
 *
 * Copy to "config.js" before editing.
 */
config = {

    // Elastic Search configuration.
    //
    // This can also be configured via the web interface and it will be stored
    // in local storage.
    elasticSearch: {
        url: "http://" + window.location.hostname + ":9200",
        size: 100
    },

    // If URL is set, will add a "Send to Dumpy" button.
    //
    // This should turn into a list of configurable items you can "send" an
    // event to.
    dumpy: {
        url: null
    }

};

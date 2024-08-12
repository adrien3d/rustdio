pub struct Station<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub fm_frequency: f32,
    pub web_url: &'a str,
}

static STATIONS: [Station; 18] = [
    Station {
        id: "bfm_business",
        name: "BFM Business",
        fm_frequency: 96.4,
        web_url: "",
    },
    Station {
        id: "cherie_fm",
        name: "Cherie FM",
        fm_frequency: 91.3,
        web_url: "",
    },
    Station {
        id: "europe_1",
        name: "Europe 1",
        fm_frequency: 104.7,
        web_url: "",
    },
    Station {
        id: "europe_2",
        name: "Europe 2",
        fm_frequency: 103.5,
        web_url: "http://europe2.lmn.fm/europe2.mp3",
    },
    Station {
        id: "fip",
        name: "FIP",
        fm_frequency: 105.1,
        web_url: "http://icecast.radiofrance.fr/fip-hifi.aac",
    },
    Station {
        id: "france_info",
        name: "France Info",
        fm_frequency: 105.5,
        web_url: "http://icecast.radiofrance.fr/franceinfo-hifi.aac",
    },
    Station {
        id: "france_inter",
        name: "France Inter",
        fm_frequency: 87.6,
        web_url: "",
    },
    Station {
        id: "france_inter_2",
        name: "France Inter Test 2",
        fm_frequency: 87.8,
        web_url: "",
    },
    Station {
        id: "le_mouv",
        name: "Le Mouv",
        fm_frequency: 92.1,
        web_url: "",
    },
    Station {
        id: "nostalgie",
        name: "Nostalgie",
        fm_frequency: 90.4,
        web_url: "https://scdn.nrjaudio.fm/adwz2/fr/30601/mp3_128.mp3",
    },
    Station {
        id: "nrj",
        name: "NRJ",
        fm_frequency: 100.3,
        web_url: "https://scdn.nrjaudio.fm/adwz2/fr/30001/mp3_128.mp3",
    },
    Station {
        id: "radio_enghien",
        name: "Station Enghien",
        fm_frequency: 98.0,
        web_url: "",
    },
    Station {
        id: "rfm",
        name: "RFM",
        fm_frequency: 103.9,
        web_url: "http://stream.rfm.fr/rfm.mp3",
    },
    Station {
        id: "rire_et_chansons",
        name: "Rire & Chansons",
        fm_frequency: 97.4,
        web_url: "https://scdn.nrjaudio.fm/adwz2/fr/30401/mp3_128.mp3",
    },
    Station {
        id: "rmc",
        name: "RMC",
        fm_frequency: 103.1,
        web_url: "http://audio.bfmtv.com/rmcradio_128.mp3",
    },
    Station {
        id: "rtl",
        name: "RTL",
        fm_frequency: 104.3,
        web_url: "http://icecast.rtl.fr/rtl-1-44-128?listen=webCwsBCggNCQgLDQUGBAcGBg",
    },
    Station {
        id: "rtl_2",
        name: "RL2",
        fm_frequency: 105.9,
        web_url: "http://icecast.rtl2.fr/rtl2-1-44-128?listen=webCwsBCggNCQgLDQUGBAcGBg",
    },
    Station {
        id: "tsf_jazz",
        name: "TSF Jazz",
        fm_frequency: 1.0,
        web_url: "https://tsfjazz.ice.infomaniak.ch/tsfjazz-high.mp3",
    },
];

impl Station<'_> {
    pub fn get_name_from_id(id: &str) -> Option<&str> {
        for station in &STATIONS {
            if station.id == id {
                return Some(station.name);
            }
        }
        None
    }

    pub fn get_fm_frequency_from_id(id: &str) -> Option<f32> {
        for station in &STATIONS {
            if station.id == id {
                return Some(station.fm_frequency);
            }
        }
        None
    }

    pub fn get_web_url_from_id(id: &str) -> Option<&str> {
        for station in &STATIONS {
            if station.id == id {
                return Some(station.web_url);
            }
        }
        None
    }
}

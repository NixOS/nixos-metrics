# !/usr/bin/env python3

# TODO: clean this up, convert it to rust, and integrate it into the main tool

import datetime
import json
import os
import sys

msPerDay = 86400000  # 1000 * 60 * 60 * 24


def toDatestamp(ms):
    return datetime.datetime.fromtimestamp(int(ms / 1000)).strftime("%Y-%m-%d")


data = {
    "pageviews": {},
    "pageviews_7day": {},
    "visitors": {},
    "visitors_7day": {},
    "sources": {},
}

for fn in os.listdir("./data/netlify"):
    f = open(f"./data/netlify/{fn}")
    jdata = json.load(f)
    f.close()
    # skip the last entry because it's partial. hadn't finished the day when it was taken
    for datum in jdata["pageviews"]["data"][:-1]:
        date = datum[0]
        # sanity check that we're getting the same values everywhere
        if date in data["pageviews"]:
            if data["pageviews"][date] != datum[1]:
                print(
                    f'data mismatch on {toDatestamp(date)} (pageviews): {data["pageviews"][date]} from before and {datum[1]} found in file {fn}'
                )
                exit(1)
        else:
            data["pageviews"][date] = datum[1]
    for datum in jdata["visitors"]["data"][:-1]:
        date = datum[0]  #
        # sanity check that we're getting the same values everywhere
        if date in data["visitors"]:
            if data["visitors"][date] != datum[1]:
                print(
                    f'data mismatch on {toDatestamp(date)} (visitors): {data["visitors"][date]} from before and {datum[1]} found in file {fn}'
                )
                exit(1)
        else:
            data["visitors"][date] = datum[1]
    current_date = jdata["pageviews"]["data"][-1][0]
    for source in jdata["sources"]["data"]:
        if source["path"] not in data["sources"]:
            data["sources"][source["path"]] = {}
        data["sources"][source["path"]][current_date] = source["count"]


def avg7day(data):
    avg = {}
    nDays = {}
    for date, datum in data.items():
        for i in range(0, 7):
            newDate = date + (i * msPerDay)
            if newDate not in avg:
                avg[newDate] = 0
                nDays[newDate] = 0
            avg[newDate] += datum / 7.0
            nDays[newDate] += 1
    for date in nDays:
        if nDays[date] != 7:
            del avg[date]
    return avg


data["pageviews_7day"] = avg7day(data["pageviews"])
data["visitors_7day"] = avg7day(data["visitors"])

graphs = {
    "pageviews": [
        {
            "label": "Pageviews",
            "x": [pair[0] for pair in sorted(data["pageviews"].items())],
            "y": [pair[1] for pair in sorted(data["pageviews"].items())],
        },
        {
            "label": "7 day avg",
            "x": [pair[0] for pair in sorted(data["pageviews_7day"].items())],
            "y": [pair[1] for pair in sorted(data["pageviews_7day"].items())],
        },
    ],
    "visitors": [
        {
            "label": "Visitors",
            "x": [pair[0] for pair in sorted(data["visitors"].items())],
            "y": [pair[1] for pair in sorted(data["visitors"].items())],
        },
        {
            "label": "7 day avg",
            "x": [pair[0] for pair in sorted(data["visitors_7day"].items())],
            "y": [pair[1] for pair in sorted(data["visitors_7day"].items())],
        },
    ],
    "sources": [
        {
            "label": source if source != "" else "direct",
            "x": [pair[0] for pair in sorted(data["sources"][source].items())],
            "y": [pair[1] for pair in sorted(data["sources"][source].items())],
        }
        for source in data["sources"]
    ],
}

json.dump(graphs, sys.stdout)

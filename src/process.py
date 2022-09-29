# !/usr/bin/env python3

# TODO: clean this up, convert it to rust, and integrate it into the main tool

import datetime
import json
import os
import sys

msPerDay = 86400000  # 1000 * 60 * 60 * 24


def toDatestamp(ms):
    return datetime.datetime.fromtimestamp(int(ms / 1000)).strftime("%Y-%m-%d")


data = {"pageviews": {}, "pageviews_7day": {}, "visitors": {}, "visitors_7day": {}}

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

avg = {}
for date, pv in data["pageviews"].items():
    for i in range(-3, 4):
        newDate = date + (i * msPerDay)
        if newDate in avg:
            avg[newDate]["pageviews"] += pv
            avg[newDate]["daysInAvg"] += 1
        else:
            avg[newDate] = {"pageviews": pv, "daysInAvg": 1}
for date in avg:
    if avg[date]["daysInAvg"] == 7:
        data["pageviews_7day"][date] = avg[date]["pageviews"] / 7.0

avg = {}
for date, pv in data["visitors"].items():
    for i in range(-3, 4):
        newDate = date + (i * msPerDay)
        if newDate in avg:
            avg[newDate]["visitors"] += pv
            avg[newDate]["daysInAvg"] += 1
        else:
            avg[newDate] = {"visitors": pv, "daysInAvg": 1}
for date in avg:
    if avg[date]["daysInAvg"] == 7:
        data["visitors_7day"][date] = avg[date]["visitors"] / 7.0

graph = [
    {
        "label": "Pageviews",
        "x": [pair[0] for pair in sorted(data["pageviews"].items())],
        "y": [pair[1] for pair in sorted(data["pageviews"].items())],
    },
    {
        "label": "PV (7 day)",
        "x": [pair[0] for pair in sorted(data["pageviews_7day"].items())],
        "y": [pair[1] for pair in sorted(data["pageviews_7day"].items())],
    },
    {
        "label": "Visitors",
        "x": [pair[0] for pair in sorted(data["visitors"].items())],
        "y": [pair[1] for pair in sorted(data["visitors"].items())],
    },
    {
        "label": "Visitors (7 day)",
        "x": [pair[0] for pair in sorted(data["visitors_7day"].items())],
        "y": [pair[1] for pair in sorted(data["visitors_7day"].items())],
    },
]

json.dump(graph, sys.stdout)

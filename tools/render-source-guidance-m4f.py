"""Validate an opt-in actual-state trace and build an offline development viewer.
Usage: python tools/render-source-guidance-m4f.py target/m4f/stage-c-trace.log
Standard library only. Opens no browser, server or application window.
"""
import json
import math
from pathlib import Path
import sys

def build(path):
    path=Path(path)
    assert path.stat().st_size<10_000_000,'trace exceeds developer viewer budget'
    scene=None;frames=[];cost=None
    for line in path.read_text(encoding='utf-8-sig').splitlines():
        for marker in ('M4F_SCENE ','M4F_FRAME ','M4F_VIEW_COST '):
            if marker not in line:continue
            record=json.loads(line.split(marker,1)[1])
            if marker=='M4F_SCENE ':
                assert scene is None;scene=record
            elif marker=='M4F_FRAME ':frames.append(record)
            else:
                assert cost is None;cost=record
    assert scene and cost and len(frames)==25
    assert len(scene['roles'])==11 and scene['roles'].count('Car')==2 and scene['roles'].count('Bogie')==3
    assert len(scene['track'])==241 and len(scene['running'])==30
    assert abs(scene['dt']-1/240)<1e-18
    for tick,frame in enumerate(frames):
        assert frame['tick']==tick and abs(frame['time']-tick/240)<1e-16
        assert len(frame['bodies'])==11 and len(frame['supports'])==30
        assert len(frame['anchors'])==len(scene['edges'])
        assert frame['max_position']<2e-5 and frame['max_velocity']<.003
        for body in frame['bodies']:
            assert len(body)==8 and all(math.isfinite(v) for v in body)
            assert abs(sum(v*v for v in body[3:7])-1)<1e-12
        for support in frame['supports']:
            assert len(support)==13 and all(math.isfinite(v) for v in support)
            assert int(support[0])==support[0] and 0<=support[0]<11
            assert abs(sum(v*v for v in support[7:10])-1)<1e-12
            gap=sum((support[1+i]-support[4+i])*support[7+i] for i in range(3))
            assert abs(gap-support[11])<1e-12
    assert math.dist(frames[0]['bodies'][0][:3],frames[-1]['bodies'][0][:3])>.1
    data={'scene':scene,'frames':frames,'cost':cost}
    encoded=json.dumps(data,separators=(',',':'),allow_nan=False).replace('<','\\u003c')
    template=Path(__file__).with_name('source-guidance-m4f-viewer.html').read_text(encoding='utf-8')
    assert template.count('__DATA__')==1
    destination=path.with_name('source-guidance-m4f.html')
    destination.write_text(template.replace('__DATA__',encoded),encoding='utf-8')
    destination.with_suffix('.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n',encoding='utf-8')
    print(f'PASS: {len(frames)} actual states, 11 bodies, 30 physical datums; {destination.stat().st_size} HTML bytes')
    print(f'Viewer: {destination.resolve()}')
    print(f'Observed native export costs: {cost}')
    return destination

if __name__=='__main__':build(sys.argv[1])

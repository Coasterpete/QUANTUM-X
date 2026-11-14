"""Independent Decimal datum/normal differences and NumPy B1 reaction systems.
Usage: python tools/verify-source-guidance-m4f.py STAGE_B_LOG
"""
from decimal import Decimal as D, localcontext
import importlib.util
import json
from pathlib import Path
import sys
import numpy as np

spec=importlib.util.spec_from_file_location('dense',Path(__file__).with_name('verify-graph-dense-m4f.py'))
dense=importlib.util.module_from_spec(spec)
spec.loader.exec_module(dense)
r=dense.retained

def verify(path):
    geometry=[]; systems=[]
    for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
        for marker,records in [('M4F_GEOMETRY ',geometry),('M4F_SYSTEM ',systems)]:
            if marker in line:
                records.append(json.loads(line.split(marker,1)[1]))
    assert len(geometry)==42 and len(systems)==12,(len(geometry),len(systems))
    worst={k:0. for k in ['p','d','dd','ddd','n','dn','ddn']}
    with localcontext() as context:
        context.prec=70
        for data in geometry:
            cell=dense.Cell([data['domain'],data['start'],data['q']])
            center=dense.dec(data['s'])-cell.s0
            offset=list(map(dense.dec,data['offset']))
            def datum(s):
                p=cell.at(s)
                return [p[i]+sum(offset[a]*p[3+3*a+i] for a in range(3)) for i in range(3)]
            def normal(s):
                p=cell.at(s)
                # Independent fourth-order, high-precision position-only P'.
                h=D('0.0000001')
                rows=[datum(s+D(i)*h) for i in [-2,-1,1,2]]
                prime=[(a-8*b+8*c-d)/(12*h) for a,b,c,d in zip(*rows)]
                b=r.cross(p[9:12],prime) if data['role']=='Running' else r.cross(prime,p[6:9])
                return r.normalized(b)
            d=r.differentiate(datum,center,D('0.000005'),list(range(-4,5)))
            n=r.differentiate(normal,center,D('0.000005'),list(range(-4,5)))
            expected=dict(zip(['p','d','dd','ddd'],[datum(center),*d]))
            expected.update(dict(zip(['n','dn','ddn'],[normal(center),*n[:2]])))
            for key,value in expected.items():
                error=max(abs(a-dense.dec(b)) for a,b in zip(value,data[key]))
                worst[key]=max(worst[key],float(error))
                assert error<D('2e-8' if key=='ddd' else '2e-9'),(data['name'],key,error)
    matrix_max=[0.,0.,0.]
    names=set()
    for data in systems:
        assert data['name'] not in names;names.add(data['name'])
        j,v,gamma,loads,acceleration=[np.array(data[k]) for k in ['j','v','gamma','loads','acceleration']]
        bodies=j.shape[1]//6
        assert bodies in (7,11) and j.shape==(6*bodies-1,6*bodies)
        singular=np.linalg.svd(j,compute_uv=False)
        for threshold in [1e-5,1e-7,1e-9]:assert np.count_nonzero(singular>threshold)==j.shape[0]
        inverse=np.tile([.01]*3+[.05]*3,bodies)
        free=np.tile([0.,-9.81,0.,0.,0.,0.],bodies)
        independent=np.linalg.solve((j*inverse)@j.T,-gamma-j@free)
        errors=[np.max(np.abs(loads-independent)),np.max(np.abs(j@acceleration+gamma)),abs(loads@(j@v))]
        assert max(errors)<1e-7,(data['name'],errors)
        assert np.max(np.abs(acceleration-free-inverse*(j.T@independent)))<1e-8
        assert np.max(np.abs(j@v))<1e-9
        matrix_max=[max(a,float(b)) for a,b in zip(matrix_max,errors)]
    summary={'geometry_cases':len(geometry),'matrix_cases':len(systems),
             'geometry_max_errors':worst,'reaction_acceleration_power_max_errors':matrix_max}
    destination=Path(path).with_suffix('.independent.json')
    destination.write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(summary,indent=2));print('PASS: source datum/normal derivatives and independent reaction systems')

if __name__=='__main__':verify(sys.argv[1])

"""Independent 70/90-digit pose-only differentiation of emitted PH polynomials.

Usage: python tools/verify-graph-dense-m4f.py LOG [FIXTURE_LOG]
No graph ODE, source RHS jets, quaternion derivative or Rust inverse is reused.
Uses the retained exact-rational finite difference weights (Decimal/Fraction).
"""
import ast
from collections import defaultdict
from decimal import Decimal as D, localcontext
import importlib.util
import json
from pathlib import Path
import sys

spec = importlib.util.spec_from_file_location('retained', Path(__file__).with_name('verify-graph-extension-m4e-c.py'))
retained = importlib.util.module_from_spec(spec)
spec.loader.exec_module(retained)

def dec(x):
    return D.from_float(x) if isinstance(x, float) else D(x)

def conjugate(q):
    return [q[0], *[-v for v in q[1:]]]

def product(a, b):
    w, x, y, z = a
    v, i, j, k = b
    return [w*v-x*i-y*j-z*k, w*i+x*v+y*k-z*j,
            w*j-x*k+y*v+z*i, w*k+x*j-y*i+z*v]

def poly(coeff, x):
    result = D(0)
    for c in reversed(coeff):
        result = result*x+c
    return result

def primitive(coeff, x):
    return poly([v/D(i+1) for i,v in enumerate(coeff)], x)*x

class Cell:
    def __init__(self, data):
        domain, p, q = data
        self.s0, self.s1, self.h = map(dec, domain)
        self.p = list(map(dec, p))
        self.q = [list(map(dec, row)) for row in q]
        self.speed = [D(0)]*15
        self.velocity = [[D(0)]*15 for _ in range(3)]
        for i,a in enumerate(self.q):
            for j,b in enumerate(self.q):
                self.speed[i+j] += sum(x*y for x,y in zip(a,b))
                v = product(product(a, [D(0),D(1),D(0),D(0)]), conjugate(b))[1:]
                for axis in range(3):
                    self.velocity[axis][i+j] += v[axis]

    def distance(self, t):
        return primitive(self.speed, t)*self.h

    def at(self, distance):
        t = distance/self.h
        for _ in range(16):
            dt = (self.distance(t)-distance)/(self.h*poly(self.speed,t))
            t -= dt
            if abs(dt)<D('1e-60'):
                break
        else:
            raise AssertionError('independent inverse failed')
        q = [poly([row[i] for row in self.q],t) for i in range(4)]
        norm = sum(v*v for v in q).sqrt()
        q = [v/norm for v in q]
        axes = []
        for i in range(3):
            a = [D(0)]*4
            a[i+1] = D(1)
            axes += product(product(q,a),conjugate(q))[1:]
        return [p+primitive(v,t)*self.h for p,v in zip(self.p,self.velocity)] + axes

def verify(paths):
    cells, queries = {}, []
    for path in paths:
        for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
            if 'M4F_CELL ' in line:
                _,name,step,index,data = line[line.index('M4F_CELL '):].split(' ',4)
                cells[name,step,index] = ast.literal_eval(data)
            elif 'M4F_QUERY ' in line:
                _,name,step,index,f,s,data = line[line.index('M4F_QUERY '):].split(' ',6)
                queries.append(((name,step,index),dec(float(f)),dec(float(s)),ast.literal_eval(data)))
    assert queries and len(cells)*5==len(queries)
    maxima = defaultdict(lambda: [0.]*7)
    limits = {}
    spacing_error = 0.
    with localcontext() as context:
        context.prec = 70
        for key,f,s,data in queries:
            cell = Cell(cells[key])
            center = cell.distance(f) if f in (0,1) else s-cell.s0
            actual = cell.at(center)
            pose, position, axes, angular = data
            pose_error = max(abs(v-dec(w)) for v,w in zip(actual,pose))
            expected = [position[i*3:i*3+3]+sum([axes[a*9+i*3:a*9+i*3+3] for a in range(3)],[]) for i in range(3)]
            nodes = list(range(9)) if f==0 else list(range(-8,1)) if f==1 else list(range(-4,5))
            observed = retained.differentiate(cell.at,center,D('0.00001'),nodes)
            finer = retained.differentiate(cell.at,center,D('0.000005'),nodes)
            if f in (0,1):
                limits[key,int(f)] = (actual,finer)
            errors = [max(abs(v-dec(w)) for v,w in zip(row,source)) for row,source in zip(finer,expected)]
            angular_observed = retained.angular(actual,finer)
            k_errors = [max(abs(v-dec(w)) for v,w in zip(row,angular[i*3:i*3+3])) for i,row in enumerate(angular_observed)]
            refinement = max(abs(a-b) for row,fine in zip(observed,finer) for a,b in zip(row,fine))
            spacing_error = max(spacing_error,float(refinement))
            assert refinement<D('1e-20'),(key,f,refinement)
            measured = [float(pose_error),*map(float,errors),*map(float,k_errors)]
            maxima[key[0]] = [max(a,b) for a,b in zip(maxima[key[0]],measured)]
            assert pose_error<D('2e-12'),(key,f,pose_error)
            assert max(errors+k_errors)<D('1e-9'),(key,f,errors,k_errors)
        # Recompute one incoming and outgoing rolling limit with 90 digits.
        for key in [('rolling','0.2','0'),('rolling','0.2','1')]:
            context.prec = 90
            c = Cell(cells[key]); end = key[2]=='0'
            center = c.distance(D(1)) if end else D(0)
            rows = retained.differentiate(c.at,center,D('0.000005'),list(range(-8,1)) if end else list(range(9)))
            context.prec = 70
            other = retained.differentiate(c.at,center,D('0.000005'),list(range(-8,1)) if end else list(range(9)))
            assert max(abs(a-b) for row,brow in zip(rows,other) for a,b in zip(row,brow))<D('1e-48')
        jumps = [D(0)]*4
        pairs = 0
        for key in cells:
            next_key = (*key[:2],str(int(key[2])+1))
            if next_key not in cells:
                continue
            left,dl = limits[key,1]
            right,dr = limits[next_key,0]
            measured = [max(abs(a-b) for a,b in zip(left,right))] + [max(abs(a-b) for a,b in zip(l,r)) for l,r in zip(dl,dr)]
            jumps = [max(a,b) for a,b in zip(jumps,measured)]
            assert measured[0]<D('2e-12') and max(measured[1:])<D('1e-9'),(key,measured)
            pairs += 1
        # Same 6 m/s offset-datum diagnostic as M4E-C, from each actual
        # polynomial branch, including the retained finite coefficient roundoff.
        left=Cell(cells['rolling','0.2','0']); right=Cell(cells['rolling','0.2','1'])
        end=left.distance(D(1)); middle=right.at(D(0))
        source=retained.datum_derivatives(limits[('rolling','0.2','1'),0][1])[1]
        acceleration=[]
        for width in map(D,('0.00006','0.00003','0.000015')):
            positions=list(map(retained.offset,(left.at(end-width),middle,right.at(width))))
            second=[(a-2*b+c)/width**2 for a,b,c in zip(*positions)]
            error=retained.norm(retained.sub(second,source))*36
            assert error<D('2e-5'),error
            acceleration.append([float(width),float(error)])
    summary = {'queries':len(queries),'cells':len(cells),'max_spacing_refinement':spacing_error,
               'adjacent_limit_pairs':pairs,'max_pose_and_order_1_2_3_limit_gaps':list(map(float,jumps)),
               'rolling_offset_acceleration_errors':acceleration,
               'max_pose_position_and_frame_orders_1_2_3_angular_orders_0_1_2':dict(maxima)}
    destination=Path(paths[0]).with_suffix('.independent.json')
    destination.write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(summary,indent=2))
    print('PASS: independent pose-only Decimal derivatives, two spacings and rolling 70/90-digit controls')

if __name__=='__main__':
    verify(sys.argv[1:])

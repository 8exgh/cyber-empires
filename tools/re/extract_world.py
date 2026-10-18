import struct,json,os,sys
src=sys.argv[1]
d=open(os.path.join(src,'MECHS.DAT'),'rb').read()
be=lambda o: struct.unpack('>h',d[o:o+2])[0]
terr={}
for f in os.listdir(src):
    if f.upper().endswith('.MAP'):
        t,n=f.split('.')[0].split('_'); terr[int(n)]=t
countries=[]
for c in range(72):
    base=0x198+c*0xEC
    name=d[base+2:base+16].split(b'\0')[0].decode('latin1')
    v=[be(base+16+2*i) for i in range(110)]
    adj=[x for x in v[9:18] if x]
    countries.append(dict(id=c+1,name=name,terrain=terr.get(c+1),adj=adj,x=v[21],y=v[22],f18=v[18],tail=v[97:]))
for c in countries:
    print(f"{c['id']:2d} {c['name']:14} {c['terrain']:5} xy=({c['x']:3d},{c['y']:3d}) adj={c['adj']} f18={c['f18']} tail={c['tail']}")
json.dump(countries,open('world.json','w'),indent=1)
# symmetry check
ids={c['id']:c for c in countries}
for c in countries:
    for a in c['adj']:
        if c['id'] not in ids[a]['adj']: print("ASYM",c['id'],a)

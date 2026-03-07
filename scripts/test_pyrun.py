# CadKit Python bridge smoke test
# Usage: in CadKit, run PYRUN and select this file.

# Base geometry
cad.line(0, 0, 40, 0)
cad.line(40, 0, 40, 30)
cad.line(40, 30, 0, 30)
cad.line(0, 30, 0, 0)

# Center circle + quadrant arcs
cad.circle(20, 15, 8)
cad.arc(20, 15, 12, 0, 90)
cad.arc(20, 15, 12, 90, 180)

# Query API checks
all_ids = cad.select()
line_ids = cad.select("line")
circle_ids = cad.select("circle")
arc_ids = cad.select("arc")

print("Total entities:", len(all_ids))
print("Lines:", len(line_ids), "Circles:", len(circle_ids), "Arcs:", len(arc_ids))

if line_ids:
    e = cad.get_entity(line_ids[0])
    print("First line entity:", e)
